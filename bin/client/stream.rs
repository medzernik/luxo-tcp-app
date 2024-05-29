use std::sync::mpsc::{Receiver, Sender};

use crate::{
    binary_message::BinaryMessage, client_commands::ServerMessageResponse, interface::ReadWrite,
};

/// Handles the incoming and outgoing messages for a client-server connection.
///
/// This function continuously reads from the outgoing channel and writes to the stream, and reads from the stream and writes to the incoming channel. It uses non-blocking reads from the outgoing channel and the stream.
///
/// # Arguments
///
/// * `stream: impl ReadWrite` - An object that implements the ReadWrite trait, typically a TCP stream.
/// * `incoming: Sender<ServerMessageResponse>` - A Sender object for the incoming channel, used to send messages to the server.
/// * `outgoing: Receiver<Message>` - A Receiver object for the outgoing channel, used to receive messages from the server.
pub fn handle_stream(
    mut stream: impl ReadWrite,
    incoming: Sender<ServerMessageResponse>,
    outgoing: Receiver<BinaryMessage>,
) -> ! {
    loop {
        // This is the stream write queue
        match outgoing.try_recv() {
            Ok(message) => {
                stream.write_all(&message.serialize()).unwrap();
            }
            Err(err) => match err {
                std::sync::mpsc::TryRecvError::Empty => (),
                std::sync::mpsc::TryRecvError::Disconnected => {
                    panic!("a critical channel error occured, channeld disconnected")
                }
            },
        }

        // While the buffer has a fairly tight limit, it can be extended if needed.
        // This should be enough for spectating a few games.
        // I wouldn't send an app's source code over it tho.
        let mut temp_buffer = vec![0; 4096];
        match stream.read(&mut temp_buffer) {
            Ok(size) => {
                if size == 0 {
                    // If the server terminates connection,
                    // then panic the thread, no point in continuing.
                    panic!("Connection closed by peer");
                }

                let message: ServerMessageResponse =
                    match BinaryMessage::deserialize(&temp_buffer).try_into() {
                        Ok(value) => value,
                        Err(err) => {
                            eprintln!("error deserializing message: {err}");
                            continue;
                        }
                    };

                incoming.send(message).unwrap();
            }
            Err(e) => {
                if let std::io::ErrorKind::WouldBlock = e.kind() {
                    // We don't care about would block, so we continue to check for incoming messages.
                    continue;
                } else {
                    panic!("{e}");
                }
            }
        }

        // This is a small delay to prevent the thread from hogging the CPU.
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

/// Handles the user input from the command line.
///
/// This function continuously reads from the standard input, parses the input into a command ID and arguments, and sends them to the `input_send` channel. The command ID is expected to be a 64-bit integer, and the arguments are the rest of the input split by whitespace.
///
/// # Arguments
///
/// * `input_send: Sender<(i64, Vec<String>)>` - A Sender object for the input channel, used to send the parsed user input to the main thread.
pub fn handle_input(input_send: Sender<String>) -> ! {
    loop {
        // Parse the input, else return it's not a proper number and continue
        let mut input = String::default();

        std::io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        if input.is_empty() {
            eprintln!("type a command!");
            continue;
        }

        input.pop();
        input_send.send(input).unwrap();
    }
}
