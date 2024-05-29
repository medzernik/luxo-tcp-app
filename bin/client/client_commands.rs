use crate::{
    binary_message::{BinaryMessage, MessageType},
    interface::{ClientData, CLEAR_TERM_SEQ},
};
use std::{string::FromUtf8Error, sync::mpsc::Sender};

/// Sends a hint to the server.
///
/// This function sends a `HINT` message to the server, indicating the client's intention to provide a hint for the current game. The message includes the hint.
///
/// # Arguments
///
/// * `outgoing: &Sender<Message>` - A reference to a Sender object, used to send messages to the server.
/// * `text: &String` - Any message to the server that has no command.
pub fn message(outgoing: &Sender<BinaryMessage>, text: String) {
    // Hint the other player
    let binary_message = BinaryMessage::new_message(text);

    outgoing.send(binary_message).unwrap();
}

/// Sends a command to the server.
///
/// This function sends a `COMMAND` message to the server, indicating the client's intention to send a command. The message includes the command.
///
/// # Arguments
///
/// * `outgoing: &Sender<Message>` - A reference to a Sender object, used to send messages to the server.
/// * `text: &String` - Any message to the server that has a command.
pub fn command(outgoing: &Sender<BinaryMessage>, text: String) {
    // Hint the other player
    let binary_message = BinaryMessage::new_command(text);
    outgoing.send(binary_message).unwrap();
}

#[derive(Debug)]
/// Possible responses from the server
pub enum ServerMessageResponse {
    Unknown,
    ID(Vec<u8>),
    Error(Vec<u8>),
    Message(Vec<u8>),
    RequestAck,
    RequestedGame,
    GameVictory,
    GameDefeat,
    GameCanceled,
}

impl ServerMessageResponse {
    /// Handles the server reply.
    ///
    /// This function takes a mutable reference to a `ClientData` object and handles the server reply based on the type of message received. It updates the client's state and prints messages to the terminal.
    ///
    /// # Arguments
    ///
    /// * `client: &mut ClientData` - A mutable reference to a `ClientData` object, representing the client's state.
    pub fn handle_server_reply(&self, client: &mut ClientData) {
        // We handle the message by matching it and then determining where to put the results.
        // In some cases, we change the interface type.
        let mut server_reply = String::new();
        let mut event_message = String::new();

        match self {
            ServerMessageResponse::Unknown => server_reply = "unknown message type".to_string(),

            ServerMessageResponse::ID(data) => {
                let id = u64::from_be_bytes(data[..].try_into().unwrap_or_default());

                client.set_id(id);

                // If ID == 0 we exit as the ID wasn't assigned by server
                if client.get_id() == 0 {
                    eprintln!("invalid password provided, exiting");
                    std::process::exit(0);
                }

                server_reply = format!("Received and set an ID from server: {id}");
            }

            ServerMessageResponse::Error(data) => {
                server_reply = String::from_utf8(data.clone())
                    .unwrap_or("Incoming data was not UTF-8".to_string());
            }

            ServerMessageResponse::Message(data) => {
                event_message = String::from_utf8(data.clone())
                    .unwrap_or("Incoming data was not UTF-8".to_string());
            }

            // Arg
            ServerMessageResponse::RequestAck => {
                server_reply = "Request was acknowledged".to_string();
            }

            ServerMessageResponse::RequestedGame => {
                println!("{CLEAR_TERM_SEQ}");
                event_message = "Game started
Use /HINT to send a hint, and /GUESS to send a guess (host sends hints and oponnent guesses)
                "
                .to_string();
            }

            ServerMessageResponse::GameVictory => {
                println!("{CLEAR_TERM_SEQ}");
                event_message = "Victory!".to_string();
            }

            ServerMessageResponse::GameDefeat => {
                println!("{CLEAR_TERM_SEQ}");
                event_message = "Defeat!".to_string();
            }

            ServerMessageResponse::GameCanceled => {
                println!("{CLEAR_TERM_SEQ}");
                server_reply = "Game was cancelled".to_string();
            }
        }

        // Some UI stuff
        println!("{CLEAR_TERM_SEQ}");
        println!(
            "
Type a command with a /COMMAND or any message to send to the server.
List of commands:

DM 
HEARTBEAT
DROP 
HINT 
GUESS 
STARTGAME
CANCEL 
REQUEST

 
        "
        );
        if !server_reply.is_empty() {
            println!("SERVER REPLY: {server_reply}");
        }
        if !event_message.is_empty() {
            println!("EVENT MESSAGE: {event_message}");
        }
    }
}

impl TryFrom<BinaryMessage> for ServerMessageResponse {
    type Error = FromUtf8Error;

    fn try_from(binary_message: BinaryMessage) -> Result<Self, FromUtf8Error> {
        match binary_message.get_type() {
            MessageType::Command => {
                // Split data to command and the binary part
                let (command, binary) = binary_message.split()?;

                // Match the command
                Ok(match command.to_ascii_uppercase().as_str() {
                    "ID" => Self::ID(binary),
                    "ERROR" => Self::Error(binary),
                    "REQUESTACK" => Self::RequestAck,
                    "REQUESTEDGAME" => Self::RequestedGame,
                    "DEFEAT" => Self::GameDefeat,
                    "CANCELED" => Self::GameCanceled,
                    "VICTORY" => Self::GameVictory,
                    _ => Self::Unknown,
                })
            }
            MessageType::Message => Ok(ServerMessageResponse::Message(
                binary_message.get_message().clone(),
            )),
            _ => Ok(Self::Unknown),
        }
    }
}
