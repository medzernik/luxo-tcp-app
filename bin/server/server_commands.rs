use std::{
    string::FromUtf8Error,
    sync::{mpsc::Sender, Arc, RwLock},
};

use crate::{
    binary_message::{BinaryMessage, MessageType},
    guess_game::GameState,
    ServerData,
};

#[derive(Debug, PartialEq, Clone)]
/// All the commands that the server can receive and interpret
pub enum ServerCommandList {
    Unknown,
    HeartBeat,
    Drop,
    DirectMessage(Vec<u8>),
    Hint(Vec<u8>),
    Guess(Vec<u8>),
    StartGame(Vec<u8>),
    Message(Vec<u8>),
    CancelGame,
    RequestOpponents,
}

#[derive(Debug)]
/// Type of error when a command executes.
///
/// ``Terminate Thread`` is a critical error that should stop the thread.
///
/// ``ErrorMessage`` is a non-critical error that can be handled.
pub enum ServerCommandError {
    TerminateThread(String),
    ErrorMessage(String),
    TerminateUser(String),
}

// Implement the Display trait for ServerCommandError
impl std::fmt::Display for ServerCommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ServerCommandError::TerminateThread(err) => write!(f, "Terminating thread: {}", err),
            ServerCommandError::ErrorMessage(err) => write!(f, "ERROR: {}", err),
            ServerCommandError::TerminateUser(err) => write!(f, "Terminating user: {}", err),
        }
    }
}

// Implement the Error trait for ServerCommandError
impl std::error::Error for ServerCommandError {}

impl ServerCommandList {
    /// Executes the server command.
    ///
    /// # Arguments
    ///
    /// * `self` - A reference to the current instance of ServerCommandList.
    /// * `server` - An Arc wrapped RwLock protected ServerData object that contains the server's state.
    /// * `local_id` - A reference to the local id.
    /// * `thread_send` - A Sender from the standard mpsc module, used to send messages to other threads.
    ///
    /// # Return
    ///
    /// Returns a [`Result`] with a [`String`] if the command was successful, otherwise a [`ServerCommandError`].
    pub fn execute(
        &self,
        server: Arc<RwLock<ServerData>>,
        local_id: &u64,
        thread_send: Sender<(u64, BinaryMessage)>,
    ) -> Result<BinaryMessage, ServerCommandError> {
        match self {
            ServerCommandList::Unknown => {
                eprintln!("Unknown command received");
                Ok(BinaryMessage::new_message(
                    "Unknown command received".to_string(),
                ))
            }
            ServerCommandList::HeartBeat => {
                println!("ID {local_id}'s heartbeat received");

                Ok(BinaryMessage::new_message(
                    "OK Heartbeat received".to_string(),
                ))
            }

            ServerCommandList::Drop => {
                let mut server_write_lock = server.write().unwrap();

                if let Some(game_id) = server_write_lock.get_game_id(*local_id) {
                    let game_clone = server_write_lock.get_game_mut_ref(game_id).unwrap().clone();

                    server_write_lock.terminate_game(game_id)?;

                    let message = BinaryMessage::new_message("MATCH CANCELED".to_string());

                    // If message is valid, send the message to the broadcast channel
                    thread_send
                        .send((game_clone.get_opponent_id(), message))
                        .map_err(|err| {
                            ServerCommandError::TerminateThread(format!(
                                "critical error sending a command to proper channel: {}",
                                err
                            ))
                        })?;
                }
                println!("DROPPING THE USER");
                if let Some(game_id) = server_write_lock.get_game_id(*local_id) {
                    server.write().unwrap().terminate_game(game_id)?;

                    server_write_lock.drop_user(*local_id)?;
                }
                Err(ServerCommandError::TerminateUser(format!(
                    "user {local_id} dropped"
                )))
            }

            ServerCommandList::DirectMessage(data) => {
                // Check if the data at least has an ID of the user
                if data.is_empty() {
                    return Err(ServerCommandError::ErrorMessage(
                        "ERROR Direct message must be at least 8 bytes long".to_string(),
                    ));
                }

                // Parse the ID
                let string_text = String::from_utf8(data.clone()).unwrap();
                let tokens: Vec<&str> = string_text.split_whitespace().collect();

                let id = match tokens[0].parse::<u64>() {
                    Ok(value) => value,
                    Err(err) => {
                        return Err(ServerCommandError::ErrorMessage(format!(
                            "ERROR Direct message must start with a valid ID: {err}"
                        )))
                    }
                };

                // Check if message is parseable, if not return error to DM sender
                let message = BinaryMessage::new_message(tokens[1].to_string());

                // If message is valid, send the message to the broadcast channel
                thread_send.send((id, message)).map_err(|err| {
                    ServerCommandError::TerminateThread(format!(
                        "critical error sending a command to proper channel: {}",
                        err
                    ))
                })?;

                // Send OK message to DM sender
                Ok(BinaryMessage::new_message(format!(
                    "OK Sent '{}' to ID {id}",
                    tokens[1]
                )))
            }
            ServerCommandList::Message(message) => {
                // Check if message is parseable, if not return error to DM sender
                let message = match String::from_utf8(message.clone()) {
                    Ok(value) => value,
                    Err(err) => {
                        return Err(ServerCommandError::ErrorMessage(format!("ERROR: {}", err)))
                    }
                };


                // Send OK back to sender
                Ok(BinaryMessage::new_message(format!(
                    "Message {message} sent"
                )))
            }

            ServerCommandList::Hint(message) => {
                let mut server_write_lock = server.write().unwrap();

                let hint = String::from_utf8(message.clone()).unwrap();

                match server_write_lock.get_game_id(*local_id) {
                    Some(game_id) => {
                        server_write_lock.update_game_hint(game_id, hint.clone())?;

                        let opponent_id = server_write_lock.get_opponent_id(*local_id)?;

                        thread_send
                            .send((
                                opponent_id,
                                BinaryMessage::new_message(format!("HINT {hint}")),
                            ))
                            .map_err(|err| {
                                ServerCommandError::TerminateThread(format!(
                                    "critical error sending a command to proper channel: {}",
                                    err
                                ))
                            })?;

                        // Send the hint to the host
                        Ok(BinaryMessage::new_message("Hint sent".to_string()))
                    }
                    None => Err(ServerCommandError::ErrorMessage(format!(
                        "ERROR no game with id {local_id} found"
                    ))),
                }
            }

            ServerCommandList::Guess(message) => {
                let mut server_write_lock = server.write().unwrap();

                match server_write_lock.get_game_id(*local_id) {
                    Some(game_id) => {
                        let game = server_write_lock.get_game_mut_ref(game_id).unwrap();
                        let game_state = game.get_game_state().clone();
                        let game_host_id = game.get_host_id();
                        let guess = String::from_utf8(message.clone()).map_err(|x| ServerCommandError::ErrorMessage(format!("ERROR: {}", x)))?;
                        let attempts = game.get_attempts();
                        
                        server_write_lock.update_game_guess(game_id, guess.clone())?;
                        
            
                        match game_state {
                            GameState::Victory => {
                                // If message is valid, send the message to the broadcast channel
                                thread_send.send((game_host_id, BinaryMessage::new_command("DEFEAT".to_string())))
                                .map_err(|err| {
                                    ServerCommandError::TerminateThread(format!(
                                        "critical error sending a command to proper channel: {}",
                                        err
                                    ))
                                })?;
            
                                server_write_lock.terminate_game(game_id)?;
            
                                Ok(BinaryMessage::new_command("VICTORY".to_string()))
                            },
                            GameState::Defeat => {
                                // If message is valid, send the message to the broadcast channel
                                thread_send.send((game_host_id, BinaryMessage::new_command("VICTORY".to_string()))).map_err(|err| {
                                    ServerCommandError::TerminateThread(format!(
                                        "critical error sending a command to proper channel: {}",
                                        err
                                    ))
                                })?;
            
                                Ok(BinaryMessage::new_command("DEFEAT".to_string()))
                            },
                            GameState::Ongoing => {
                                // If message is valid, send the message to the broadcast channel
                                thread_send.send((game_host_id, BinaryMessage::new_message(format!("Guess {guess} IS INCORRECT, {} ATTEMPTS LEFT", attempts)))).map_err(|err| {
                                    ServerCommandError::TerminateThread(format!(
                                        "critical error sending a command to proper channel: {}",
                                        err
                                    ))
                                })?;
            
                                Ok(BinaryMessage::new_message(format!(
                                    "OK GUESS {guess} IS INCORRECT, {} ATTEMPTS LEFT",
                                    attempts
                                )))
                            },
                        }
                    },
                    None => {
                        Err(ServerCommandError::ErrorMessage(format!(
                            "CANCELED Game where local player ID {local_id} is trying to guess, does not exist. cancelling match"
                        )))
                    },
                }
            }

            // Args: ID, Secret
            ServerCommandList::StartGame(data) => {
                // Check if the data at least has an ID of the user
                if data.is_empty() {
                    return Err(ServerCommandError::ErrorMessage(
                        "ERROR Direct message must be at least 8 bytes long".to_string(),
                    ));
                }

                // Parse the ID
                let string_text = String::from_utf8(data.clone()).unwrap();
                let tokens: Vec<&str> = string_text.split_whitespace().collect();

                if tokens.len() != 2 {
                    return Err(ServerCommandError::ErrorMessage(
                        "ERROR Direct message must have 2 arguments".to_string(),
                    ));
                }

                let opponent_id = match tokens[0].parse::<u64>() {
                    Ok(value) => value,
                    Err(err) => {
                        return Err(ServerCommandError::ErrorMessage(format!(
                            "ERROR Direct message must start with a valid ID: {err}"
                        )))
                    }
                };

                // Check if message is parseable, if not return error to DM sender
                let secret = tokens[1].to_string();

                server
                    .write()
                    .unwrap()
                    .start_game(*local_id, opponent_id, secret.clone())?;

                let command = BinaryMessage::new_command("REQUESTEDGAME".to_string());

                // If message is valid, send the message to the broadcast channel
                thread_send.send((opponent_id, command)).map_err(|err| {
                    ServerCommandError::TerminateThread(format!(
                        "critical error sending a command to proper channel: {}",
                        err
                    ))
                })?;

                Ok(BinaryMessage::new_command("REQUESTACK".to_string()))
            }

            ServerCommandList::CancelGame => {
                let mut server_write_lock = server.write().unwrap();

                match server_write_lock.get_game_id(*local_id) {
                    Some(game_id) => {
                        // Again, the game must exist at this point and we have a mut lock so we can safely unwrap here.
                        let game_clone =
                            server_write_lock.get_game_mut_ref(game_id).unwrap().clone();

                        // Terminate the game
                        server_write_lock.terminate_game(game_id)?;

                        let message = BinaryMessage::new_message("MATCH CANCELED".to_string());

                        // If message is valid, send the message to the broadcast channel
                        thread_send
                            .send((game_clone.get_opponent_id(), message))
                            .map_err(|err| {
                                ServerCommandError::TerminateThread(format!(
                                    "critical error sending a command to proper channel: {}",
                                    err
                                ))
                            })?;

                        Ok(BinaryMessage::new_command("CANCELED".to_string()))
                    }
                    None => Err(ServerCommandError::ErrorMessage(format!(
                        "user {local_id} doesn't participate in a game, cannot terminate"
                    ))),
                }
            }
            ServerCommandList::RequestOpponents => {
                match server.read().unwrap().get_opponents(*local_id) {
                    Some(opponents) => Ok(BinaryMessage::new_message(format!(
                        "OPPONENT LIST {:?}",
                        opponents
                    ))),
                    None => Err(ServerCommandError::ErrorMessage(
                        "ERROR No opponents found".to_string(),
                    )),
                }
            }  
        
        }
    }
}

impl TryFrom<BinaryMessage> for ServerCommandList {
    type Error = FromUtf8Error;

    fn try_from(binary_message: BinaryMessage) -> Result<Self, Self::Error> {
        Ok(match binary_message.get_type() {
            MessageType::Command => {
                // Split data to command and the binary part
                let (command, binary) = binary_message.split()?;

                match command.to_ascii_uppercase().as_str() {
                    "DM" => Self::DirectMessage(binary),
                    "HEARTBEAT" => Self::HeartBeat,
                    "DROP" => Self::Drop,
                    "HINT" => Self::Hint(binary),
                    "GUESS" => Self::Guess(binary),
                    "STARTGAME" => Self::StartGame(binary),
                    "CANCEL" => Self::CancelGame,
                    "REQUEST" => Self::RequestOpponents,
                    _ => Self::Unknown,
                }
            }
            MessageType::Message => Self::Message(binary_message.get_message().clone()),

            _ => ServerCommandList::Unknown,
        })
    }
}
