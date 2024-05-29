use std::{
    io::{Read, Write},
    net::TcpStream,
    os::unix::net::UnixStream,
};

use crate::{
    client_commands::{command, message},
    stream::{handle_input, handle_stream},
    ConnectionMode,
};

/// Loop rate on UI that refreshes automatically (including sending data to server)
const LOOP_RATE_MS: u64 = 500;
pub const CLEAR_TERM_SEQ: &str = "\x1B[2J\x1B[1;1H";

/// Helper trait to generalize UNIX and TCP implementation
pub trait ReadWrite: Read + Write + Send + Sync {}

/// Needs to be implemented to be able to have multiple threads going
impl<T: Read + Write + Send + Sync> ReadWrite for T {}

/// Main client data that shows which interface is shown and client's ID.
pub struct ClientData {
    client_id: u64,
}

impl ClientData {
    /// Creates a new client, depending on whether it's a TCP or UNIX client.
    pub fn new(mode: ConnectionMode, connection_endpoint: String) -> (Box<dyn ReadWrite>, Self) {
        (
            match &mode {
                ConnectionMode::TCP => {
                    // Just simply using the same variable.
                    let ip_address = format!("127.0.0.1:{connection_endpoint}");
                    let stream = TcpStream::connect(ip_address).unwrap();
                    stream
                        .set_read_timeout(Some(std::time::Duration::from_millis(300)))
                        .unwrap();
                    Box::new(stream)
                }
                ConnectionMode::UNIX => {
                    let socket = format!("/tmp/{connection_endpoint}");
                    let stream = UnixStream::connect(socket).unwrap();
                    // UNIX is by default blocking, so we set the nonblocking read here.
                    stream.set_nonblocking(true).unwrap();

                    Box::new(stream)
                }
            },
            // Sets the user ID to 0, also the screen to the login variant
            Self {
                client_id: u64::default(),
            },
        )
    }

    /// Set the id for the client.
    pub fn set_id(&mut self, client_id: u64) {
        self.client_id = client_id;
    }

    /// Get client's ID.
    pub fn get_id(&mut self) -> u64 {
        self.client_id
    }

    pub fn await_input(&mut self, stream: Box<dyn ReadWrite>) -> ! {
        // Create the 3 text event buffers.

        // Create 3 channel pairs for separate thread-based events.
        // 1st is all incoming commands from the server.
        // 2nd is all outgoing commands to the server.
        // 3rd is an input handler.
        let (incoming_send, incoming_recv) = std::sync::mpsc::channel();
        let (outgoing_send, outgoing_recv) = std::sync::mpsc::channel();
        let (input_send, input_recv) = std::sync::mpsc::channel();

        // Thread spawn oncoming stream for command management.
        // This thread handles both incoming and outgoing commands. We just read/write to channel.
        std::thread::spawn(move || {
            handle_stream(stream, incoming_send, outgoing_recv);
        });

        // Thread spawn for any input handling. This makes sure the terminal isn't blocked.
        std::thread::spawn(move || {
            handle_input(input_send);
        });

        println!("{CLEAR_TERM_SEQ}");
        println!(
            "
Enter the password for the server, then press ENTER:
        "
        );

        // Main event loop
        loop {
            // Check if we received a message from the server, if not then continue. If yes, handle it
            match incoming_recv.try_recv() {
                Ok(response) => response.handle_server_reply(self),
                Err(err) => {
                    // If a channel is disconnected, something went terribly wrong and we need to panic.
                    // Else, we just ignore that the channel is empty.
                    if let std::sync::mpsc::TryRecvError::Disconnected = err {
                        panic!("critical error, incoming channel disconnected!")
                    }
                }
            }

            // `action_number` is the parsed value from user input. We set it to -1 if no input is detected.
            // This allows us to properly refresh the screen on a loop with new data.
            // (example: spectator screen refreshes data regularly)
            // `arguments` is a parsed and split_by_whitespace vector of the arguments of each command.

            let data: String = match input_recv.try_recv() {
                Ok(value) => value,
                Err(err) => {
                    match err {
                        std::sync::mpsc::TryRecvError::Disconnected => {
                            // If a channel is disconnected, something went terribly wrong and we need to panic.
                            // Else, we just ignore that the channel is empty.
                            panic!("critical error, incoming channel disconnected!")
                        }
                        _ => String::new(),
                    }
                }
            };

            if data.is_empty() {
                std::thread::sleep(std::time::Duration::from_millis(LOOP_RATE_MS));
                continue;
            }

            // /COMMAND
            if let Some(stripped) = data.strip_prefix('/') {
                command(&outgoing_send, stripped.to_string());
                println!("sending command");
            } else {
                message(&outgoing_send, data);
                println!("sending message");
            }

            // Wait a while to not spam the UI
            std::thread::sleep(std::time::Duration::from_millis(LOOP_RATE_MS));
        }
    }
}
