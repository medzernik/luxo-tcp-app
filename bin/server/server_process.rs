use std::net::TcpListener;
use std::os::unix::net::UnixListener;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::sync::{Arc, RwLock};

use crate::binary_message::BinaryMessage;

use crate::server_commands::{ServerCommandError, ServerCommandList};
use crate::{Broadcast, ReadWrite, ServerData, ServerType, SLEEP_DELAY_MS};

/// This function handles data communication for a server.
///
/// # Arguments
///
/// * `server` - An Arc wrapped RwLock protected ServerData object that contains the server's state.
/// * `stream` - A mutable reference to an object implementing the ReadWrite trait, typically a network stream.
/// * `thread_recv` - A Receiver from the standard mpsc module, used to receive messages from other threads.
/// * `thread_send` - A Sender from the standard mpsc module, used to send messages to other threads.
///
/// # Behavior
///
/// The function first validates the user. If the validation fails, it prints an error message and returns.
/// Then it enters a loop where it tries to receive a message from `thread_recv`. If a message is received and the id matches `local_id`,
/// it sends a response. If an error occurs during receiving, it checks if the error is because the receiver is empty. If it's not, it prints an error message and returns.
/// Then it tries to process the stream. If the processing is successful, it checks if a command is returned. If a command is returned, it executes the command.
/// If the execution is successful, it sends a response. If an error occurs during execution, it checks the type of the error and acts accordingly.
/// If no command is returned, it continues to the next iteration. If an error occurs during processing, it prints an error message and returns.
/// At the end of each iteration, it sleeps for [SLEEP_DELAY_MS] milliseconds.
fn client_connection(
    server: Arc<RwLock<ServerData>>,
    mut stream: impl ReadWrite,
    thread_recv: Receiver<(u64, BinaryMessage)>,
    thread_send: Sender<(u64, BinaryMessage)>,
) {
    // Validate the user and get the local id
    let mut local_id = match validate_user(&mut stream, server.clone()) {
        Ok(id) => id,
        Err(error) => {
            // If unsuccessful, print an error message and terminate the stream for the client.
            eprintln!("error validating user: {error}");
            return;
        }
    };

    loop {
        // Try to receive any messages that could have arrived on the broadcast channel.
        // If yes, send a response to the client the id of the message matches the current client.
        match thread_recv.try_recv() {
            Ok((id, value)) => {
                if id == local_id {
                    println!("got a message with id: {id}, value: {value:?}");

                    send_response(&mut stream, &value, server.clone(), local_id)
                }
            }
            Err(err) => {
                if err == TryRecvError::Disconnected {
                    eprintln!("error {err}");
                    return;
                }
            }
        }

        // Check whether we got a message from the client
        match process_stream(&mut stream, server.clone(), &mut local_id) {
            Ok(value) => {
                if let Some(command) = value {
                    match command.execute(server.clone(), &local_id, thread_send.clone()) {
                        Ok(value) => {
                            send_response(&mut stream, &value, server.clone(), local_id);
                        }
                        Err(err) => {
                            match err {
                                // If we encounter a critical error, we need to terminate server-side
                                ServerCommandError::TerminateThread(message) => {
                                    eprintln!("a critical error has occured: {message}, terminating thread");
                                    return;
                                }
                                // Otherwise, print the error to the user so he acknowledges
                                ServerCommandError::ErrorMessage(message) => {
                                    send_response(
                                        &mut stream,
                                        &BinaryMessage::new_message(message),
                                        server.clone(),
                                        local_id,
                                    );
                                }
                                ServerCommandError::TerminateUser(message) => {
                                    eprintln!("Terminating thread: {message}");
                                    return;
                                }
                            }
                        }
                    }
                }
            }
            Err(err) => {
                eprint!("critical error in parsing stream or command parsing: {err}");
                return;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(SLEEP_DELAY_MS));
    }
}

/// Validating client and assigning id.
///
/// # Arguments
///
/// * `stream: &mut impl ReadWrite` - A mutable reference to an object that implements the `ReadWrite` trait. This is the stream from which data is read.
/// * `server: Arc<RwLock<ServerData>>` - An `Arc<RwLock<ServerData>>` that allows multiple threads to safely share and modify the `ServerData` object.
/// * `local_id: &mut u64` - A mutable reference to the ID of the client from which data is being read.
///
/// # Returns
///
/// * [`bool`] - if the user is validated, i.e. correct password received, `true` is returned. If the user is not validated, `false` is returned.
fn validate_user(
    stream: &mut impl ReadWrite,
    server: Arc<RwLock<ServerData>>,
) -> Result<u64, ServerCommandError> {
    loop {
        match process_stream(stream, server.clone(), &mut 0) {
            Ok(value) => match value {
                Some(command) => match command {
                    ServerCommandList::Message(pass) => {
                        let message = match String::from_utf8(pass.clone()) {
                            Ok(value) => value,
                            Err(err) => {
                                return Err(ServerCommandError::ErrorMessage(format!(
                                    "ERROR: {}",
                                    err
                                )))
                            }
                        };

                        let validated = server.read().unwrap().validate_password(&message);
                        println!("received password {message}, validated: {validated}");

                        match validated {
                            true => {
                                let local_id = server.write().unwrap().add_user();
                                println!("sending localId: {}", local_id);

                                send_response(
                                    stream,
                                    &BinaryMessage::new_message(format!("ID {local_id}")),
                                    server.clone(),
                                    local_id,
                                );
                                println!(
                                    "Added id {local_id} - Server data: {:?}",
                                    server.read().unwrap().connected_users
                                );
                                return Ok(local_id);
                            }
                            false => {
                                send_response(
                                    stream,
                                    &BinaryMessage::new_message(
                                        "ERROR password incorrect".to_string(),
                                    ),
                                    server.clone(),
                                    0,
                                );
                                return Err(ServerCommandError::TerminateThread(
                                    "Password incorrect".to_string(),
                                ));
                            }
                        };
                    }
                    _ => {
                        send_response(
                            stream,
                            &BinaryMessage::new_message("ERROR command incorrect".to_string()),
                            server.clone(),
                            0,
                        );
                        return Err(ServerCommandError::TerminateThread(
                            "Command incorrect".to_string(),
                        ));
                    }
                },
                None => {
                    continue;
                }
            },
            Err(err) => {
                eprint!("critical error in parsing stream or command parsing: {err}");
                return Err(ServerCommandError::TerminateThread(
                    "Critical error in parsing stream or command parsing".to_string(),
                ));
            }
        }
    }
}

/// Send data to TCP stream.
///
/// # Arguments
///
/// * `stream: &mut impl ReadWrite` - A mutable reference to an object that implements the `ReadWrite` trait. This is the stream from which data is read.
/// * `response: &Message` - A reference to the data format to be sent to the TCP stream.
/// * `server: Arc<RwLock<ServerData>>` - An `Arc<RwLock<ServerData>>` that allows multiple threads to safely share and modify the `ServerData` object.
/// * `local_id: &mut u64` - A mutable reference to the ID of the client from which data is being read.
///
/// # Returns
///
/// * [`Result<Option<ServerCommandList>, String>`] - If data is successfully read and processed, an `Option<ServerCommandList>` is returned. If an error occurs, a [`String`] error message is returned.
fn send_response(
    stream: &mut impl ReadWrite,
    response: &BinaryMessage,
    server: Arc<RwLock<ServerData>>,
    local_id: u64,
) {
    println!("writing a response to client: {:?}", response);
    println!(
        "writing a response to client: {:#?}",
        String::from_utf8(response.get_message().clone()).unwrap()
    );
    if let Err(err) = stream.write_all(&response.serialize()) {
        if let Err(error) = server.write().unwrap().drop_user(local_id) {
            eprintln!("critical error writing a response: {err} and {error}");
        }
    }
}

/// Processes a stream of data from a client.
///
/// This function reads data from a client, processes it, and returns a `ServerCommandList` or an error. It runs in an infinite loop, constantly trying to read data from the stream. If data is successfully read, it is processed and a `ServerCommandList` is returned. If an error occurs while reading the data, the error is handled and returned.
///
/// # Arguments
///
/// * `stream: &mut impl ReadWrite` - A mutable reference to an object that implements the `ReadWrite` trait. This is the stream from which data is read.
/// * `server: Arc<RwLock<ServerData>>` - An `Arc<RwLock<ServerData>>` that allows multiple threads to safely share and modify the `ServerData` object.
/// * `local_id: &mut u64` - A mutable reference to the ID of the client from which data is being read.
///
/// # Returns
///
/// * [`Result<Option<ServerCommandList>, String>`] - If data is successfully read and processed, an `Option<ServerCommandList>` is returned. If an error occurs, a [`String`] error message is returned.
fn process_stream(
    stream: &mut impl ReadWrite,
    server: Arc<RwLock<ServerData>>,
    local_id: &mut u64,
) -> Result<Option<ServerCommandList>, ServerCommandError> {
    // The buffer could be extended if needed,
    // but this should be enough for most cases, unless you spectate a 1000 games.
    let mut temp_buffer: Vec<u8> = vec![0u8; 4096];

    // Read the stream and try to parse the message
    match stream.read(&mut temp_buffer) {
        Ok(size) => {
            if size == 0 {
                if let Err(err) = server.write().unwrap().drop_user(*local_id) {
                    return Err(ServerCommandError::TerminateThread(format!(
                        "error removing user: {err}"
                    )));
                }
                return Err(ServerCommandError::TerminateThread(
                    "connection closed by peer".to_string(),
                ));
            }

            println!(
                "Buffer contents: {:#?}",
                String::from_utf8(temp_buffer.clone()).unwrap()
            );

            if String::from_utf8(temp_buffer.clone())
                .unwrap()
                .starts_with("GET / HTTP/1.1")
            {
                println!("GOT A BROwOSER");
                let spectator = server.read().unwrap().get_spectator_data();
                let spectator = format!("{:#?}", spectator);
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8\r\n\r\n<!DOCTYPE html><html><head><title>Server</title></head><body><h1>Server</h1><p>{spectator}</p></body></html>");
                stream.write_all(response.as_bytes()).unwrap();
                return Ok(None);
            }

            // Parse the message or return an empty one
            let message: ServerCommandList =
                match BinaryMessage::deserialize(&temp_buffer).try_into() {
                    Ok(value) => value,
                    Err(err) => {
                        eprintln!("error deserializing message: {err}");
                        return Ok(None);
                    }
                };
            Ok(Some(message))
        }
        Err(e) => {
            // If the error is due to the stream being empty, return None
            if let std::io::ErrorKind::WouldBlock = e.kind() {
                Ok(None)
            } else {
                if let Err(err) = server.write().unwrap().drop_user(*local_id) {
                    eprintln!("error removing user: {err}");
                }
                Err(ServerCommandError::TerminateThread(format!(
                    "critical error in stream: {e}"
                )))
            }
        }
    }
}

/// This server estabilishes the type of the server and runs the thread pool of data processing.
pub fn run_server(server_data: ServerData, connection_endpoint: &str) -> std::io::Result<()> {
    // address can be either UNIX pipe path or IP address
    let address;

    // prevent a deadlock by caching the value
    let server_type = server_data.server_type.clone();

    // Create an Arc RWLock for multithreaded use of server data
    let server_data = Arc::new(RwLock::from(server_data));

    // Create a channel to communicate from threads to dispatcher.
    let (thread_send, thread_recv) = std::sync::mpsc::channel();

    // Create a broadcast from the dispatcher to all threads to catch messages.
    let broadcast = Arc::new(RwLock::from(Broadcast::new()));

    // Clone the broadcast
    let broadcast_clone = broadcast.clone();

    // Thread handles, creating the list of handles to await for join (finish) of all of them so this function will block.
    let mut thread_handles = vec![];

    // Launch the dispatcher with the appropriate channels as a separate thread.
    // Push the handle to the thread_handles.
    let handle = std::thread::spawn(move || {
        dispatch(thread_recv, broadcast_clone.clone());
    });
    thread_handles.push(handle);

    // Print the server password to the console.
    println!(
        "Current server password is set to: {}",
        server_data.read().unwrap().password
    );

    // Check the type of the server
    match server_type {
        ServerType::TCP => {
            // Address is hardcoded for localhost, but could be changed with an argument. Port is based on the argument.
            address = format!("127.0.0.1:{connection_endpoint}");
            println!("Starting TCP server on address {address}");

            // Start listening on the address.
            let listener = TcpListener::bind(address)?;

            // Accept connections and process them serially - creating a thread for each one.
            // Also set a read timeout in case a client drops connection
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                stream
                    .set_read_timeout(Some(std::time::Duration::from_millis(SLEEP_DELAY_MS)))
                    .unwrap();

                // Rust needs that we clone the ARCs here to send them to the thread.
                let server_clone = server_data.clone();
                let broadcast_clone = broadcast.clone();
                let thread_send_clone = thread_send.clone();

                // Launch each connection in a separate thread,
                // giving them the communication channels to the dispatcher and from dispatcher.
                let handle = std::thread::spawn(move || {
                    let subscription;
                    // We need to drop the broadcast write lock, so we scope it.
                    {
                        let mut broadcast = broadcast_clone.write().unwrap();
                        subscription = broadcast.subscribe();
                    }
                    client_connection(
                        server_clone,
                        stream,
                        subscription,
                        thread_send_clone.clone(),
                    );
                });
                // Push each TCP stream to the pool
                thread_handles.push(handle);
            }
        }
        ServerType::UNIX => {
            address = format!("/tmp/{connection_endpoint}");
            println!("starting server on UNIX socket {address}");

            // The UNIX pipe doesn't get removed automatically, so we remove it here if it exists.
            std::fs::remove_file(&address).unwrap_or_else(|err| match err.kind() {
                std::io::ErrorKind::NotFound => (),
                _ => eprintln!("error removing socket: {}", err),
            });

            // We bind to the UNIX pipe
            let listener = UnixListener::bind(address)?;

            // And then add each concurrent stream to a separate thread
            for stream in listener.incoming() {
                // We have to unwrap here due to rust needing a proper binding
                // and not an .unwrap() later that would move the value.
                let stream = stream.unwrap();

                // Set a read timeout for users that drop out
                stream
                    .set_read_timeout(Some(std::time::Duration::from_millis(SLEEP_DELAY_MS)))
                    .unwrap();

                // Internal clones of ARCs for cross-thread send
                let server_clone = server_data.clone();
                let broadcast_clone = broadcast.clone();
                let thread_send_clone = thread_send.clone();
                let handle = std::thread::spawn(move || {
                    // Dropping the write lock by scoping the lock
                    let subscription;
                    {
                        let mut broadcast = broadcast_clone.write().unwrap();
                        subscription = broadcast.subscribe();
                    }
                    client_connection(
                        server_clone,
                        stream,
                        subscription,
                        thread_send_clone.clone(),
                    );
                });

                // Push the thread to a pool
                thread_handles.push(handle);
            }
        }
    }

    // Wait for all threads to join (finish)
    for handle in thread_handles {
        handle.join().unwrap();
    }
    Ok(())
}

/// Continuously dispatches messages received from threads to a broadcast channel.
///
/// This function runs in an infinite loop, constantly trying to receive messages from the `thread_dispatch_recv` channel. When a message is received, it is sent to the `broadcast` channel. If an error occurs while trying to receive a message, the error is handled and logged.
///
/// # Arguments
///
/// * `thread_dispatch_recv: Receiver<(u64, String)>` - The receiving end of a channel from which messages sent by threads are received. Each message is a tuple where the first element is the ID of the thread that sent the message and the second element is the message itself.
/// * `broadcast: Arc<RwLock<Broadcast>>` - An `Arc<RwLock<Broadcast>>` that allows multiple threads to safely share and modify the `Broadcast` object. Messages received from `thread_dispatch_recv` are sent to this broadcast channel.
pub fn dispatch(
    thread_dispatch_recv: Receiver<(u64, BinaryMessage)>,
    broadcast: Arc<RwLock<Broadcast>>,
) {
    loop {
        match thread_dispatch_recv.try_recv() {
            Ok(message) => {
                println!(
                    "received a message from thread ID {}, contains: {:?}",
                    message.0, message.1
                );
                // When we get a message to send to another thread, broadcast it to all with the id of the receiver.
                broadcast.write().unwrap().broadcast(message);
                println!("sent the message to broadcast channel");
            }
            Err(err) => {
                if let TryRecvError::Disconnected = err {
                    eprint!("error on dispatch recv: {err}");
                }
            }
        }
        // We wait a bit to not hog the CPU
        std::thread::sleep(std::time::Duration::from_millis(SLEEP_DELAY_MS));
    }
}
