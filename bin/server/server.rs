use std::{
    io::{Read, Write},
    sync::mpsc::{Receiver, Sender},
};

use binary_message::BinaryMessage;
use guess_game::{Game, GameState};
use server_commands::ServerCommandError;

mod binary_message;
mod guess_game;
mod server_commands;
mod server_process;

#[derive(Debug)]
/// This is basically a custom broadcast due to the limitation of not using tokio's types.
struct Broadcast {
    senders: Vec<(Sender<(u64, BinaryMessage)>, u64)>,
}

impl Broadcast {
    /// Creates a new [`Broadcast`].
    fn new() -> Self {
        Broadcast {
            senders: Vec::default(),
        }
    }

    /// Equivalent of subscribing to a broadcast channel.
    fn subscribe(&mut self) -> Receiver<(u64, BinaryMessage)> {
        let (tx, rx) = std::sync::mpsc::channel();

        // Generate a new channelID for each user that subscribes (gets added)
        let id = self
            .senders
            .iter()
            .map(|(_, item_id)| *item_id)
            .max()
            .unwrap_or(0)
            + 1;

        self.senders.push((tx, id));
        rx
    }

    /// Equivalent of unsubscribing from a broadcast channel.
    fn unsubscribe(&mut self, channels_to_remove: Vec<u64>) {
        channels_to_remove
            .iter()
            .for_each(|remove_id| self.senders.retain(|(_, id)| id != remove_id));
    }

    /// Broadcast the message to all receivers.
    fn broadcast(&mut self, message: (u64, BinaryMessage)) {
        let mut channels_to_remove = vec![];

        for (sender, id) in &self.senders {
            match sender.send(message.clone()) {
                Ok(_) => {
                    println!("Sent a message {sender:?} {message:#?}");
                }
                Err(e) => {
                    // If we have dead thread, remove it's old channel.
                    eprintln!(
                        "Failed to send message: {e}, {:#?}, removing channel",
                        sender
                    );
                    channels_to_remove.push(*id);
                }
            }
        }
        self.unsubscribe(channels_to_remove);
    }
}

#[derive(Debug, PartialEq, Clone)]
/// Type of server to run.
pub enum ServerType {
    TCP,
    UNIX,
}

#[derive(Debug)]
/// Data for the server to keep track of, mainly list of connected users, ongoing games and password.
pub struct ServerData {
    connected_users: Vec<u64>,
    password: String,
    server_type: ServerType,
    ongoing_games: Vec<Game>,
}

impl ServerData {
    /// Creates a new [`ServerData`].
    pub fn new(password: String, server_type: ServerType) -> Self {
        Self {
            connected_users: vec![],
            password,
            server_type,
            ongoing_games: vec![],
        }
    }

    /// Gets all valid opponents. Excludes spectators
    pub fn get_opponents(&self, local_id: u64) -> Option<Vec<u64>> {
        if self.connected_users.is_empty() {
            None
        } else {
            Some(
                self.connected_users
                    .iter()
                    .filter(|x| **x != local_id)
                    .cloned()
                    .collect(),
            )
        }
    }

    /// Starts a game, requires id of host, id of guest and the secret word that will be guessed
    pub fn start_game(
        &mut self,
        id_host: u64,
        id_guest: u64,
        secret: String,
    ) -> Result<u128, ServerCommandError> {
        // Check if user matched exists
        if !self.connected_users.iter().any(|id| id_guest == *id) {
            return Err(ServerCommandError::ErrorMessage(format!(
                "ERROR user {id_guest} doesn't exist, cannot begin game"
            )));
        }

        // Check if user is trying to match with himself
        if id_guest == id_host {
            return Err(ServerCommandError::ErrorMessage(format!(
                "ERROR user {id_guest} cannot match with himself!"
            )));
        }

        // Check if user is already in a game
        let new_game = Game::new(id_host, id_guest, secret);
        let id = new_game.get_game_id();

        // Starts the game by pushing it into the ongoing games
        self.ongoing_games.push(new_game);

        Ok(id)
    }

    pub fn get_game_id(&self, id: u64) -> Option<u128> {
        for game in self.ongoing_games.iter() {
            if game.get_host_id() == id {
                return Some(game.get_game_id());
            }
            if game.get_opponent_id() == id {
                return Some(game.get_game_id());
            }
        }
        None
    }
    pub fn get_game_mut_ref(&mut self, id: u128) -> Option<&mut Game> {
        self.ongoing_games
            .iter_mut()
            .find(|game| game.get_game_id() == id)
    }

    /// Gets an oponent ID from a game. Each game must have an opponent and a host.
    pub fn get_opponent_id(&self, player_id: u64) -> Result<u64, ServerCommandError> {
        if let Some(game) = self
            .ongoing_games
            .iter()
            .find(|x| player_id == x.get_host_id())
        {
            return Ok(game.get_opponent_id());
        }
        Err(ServerCommandError::ErrorMessage(
            "game has no oppponent".to_string(),
        ))
    }

    /// Gets a host ID from a game. Each game must have an host and an opponent.
    pub fn get_host_id(&self, player_id: u64) -> Result<u64, ServerCommandError> {
        if let Some(game) = self
            .ongoing_games
            .iter()
            .find(|x| player_id == x.get_opponent_id())
        {
            return Ok(game.get_host_id());
        }
        Err(ServerCommandError::ErrorMessage(
            "game has no host".to_string(),
        ))
    }

    pub fn get_spectator_data(&self) -> Vec<Game> {
        self.ongoing_games.clone()
    }

    pub fn terminate_game(&mut self, id: u128) -> Result<(), ServerCommandError> {
        if !self.ongoing_games.iter().any(|x| id == x.get_game_id()) {
            return Err(ServerCommandError::ErrorMessage(format!(
                "user {id} doesn't exist"
            )));
        }

        self.ongoing_games.retain(|x| x.get_game_id() != id);
        println!("removed game id {id}");
        Ok(())
    }

    /// Updates a game guess, determining if a game is won or lost, returns back the game state reference
    pub fn update_game_guess(
        &mut self,
        id: u128,
        guess: String,
    ) -> Result<&GameState, ServerCommandError> {
        for game in self.ongoing_games.iter_mut() {
            if game.get_game_id() == id {
                // If the word is guessed, then change the state to Victory
                if guess.to_lowercase() == game.get_secret().to_lowercase() {
                    game.set_game_state(GameState::Victory);
                    return Ok(game.get_game_state());
                }
                // Otherwise lower the attempts and update last guess for spectators
                game.set_attempts(game.get_attempts() - 1);
                game.set_last_guess(guess);

                // If we are out of attempts, we mark the game as lost for the guesser
                if game.get_attempts() == 0 {
                    game.set_game_state(GameState::Defeat);
                    return Ok(game.get_game_state());
                }

                // Otherwise just return the current game state reference
                return Ok(game.get_game_state());
            }
        }
        Err(ServerCommandError::ErrorMessage(
            "Game does not exist, cannot update guess".to_string(),
        ))
    }

    /// Updates the last game hint for the game state and for spectators
    pub fn update_game_hint(&mut self, id: u128, hint: String) -> Result<(), ServerCommandError> {
        for game in self.ongoing_games.iter_mut() {
            if game.get_game_id() == id {
                // Set the last hint and return OK
                game.set_last_hint(hint);
                return Ok(());
            }
        }
        Err(ServerCommandError::ErrorMessage(
            "game does not exist, cannot update hint".to_string(),
        ))
    }

    /// Validates whether a password provided matches the server password, returning true if yes.
    pub fn validate_password(&self, password_external: &str) -> bool {
        self.password == password_external
    }

    /// Verifies whether a user with an ID X exists, returning true if yes.
    pub fn user_exists(&self, id: u64) -> bool {
        self.connected_users
            .iter().copied()
            .any(|x| x == id)
    }

    /// Adds a user, generating a new user ID that is always higher than the last highest connected user, returning the ID.
    /// Also pushes the user to the added users
    pub fn add_user(&mut self) -> u64 {
        let id = self
            .connected_users
            .iter().copied()
            .max()
            .unwrap_or(0)
            + 1;

        // Add the user
        self.connected_users.push(id);
        id
    }

    /// Drops the user from registered list. If the user already was removed, remove an error saying he was removed.
    pub fn drop_user(&mut self, id: u64) -> Result<(), ServerCommandError> {
        if !self.user_exists(id) {
            return Err(ServerCommandError::ErrorMessage(format!(
                "user {id} doesn't exist"
            )));
        }
        // Remove the user
        self.connected_users.retain(|list_id| id != *list_id);

        // We also log the change in the server's terminal
        println!("removing user id: {id}");

        // We can't return a message here as it would be sent to a non-existing user, so we return ().
        Ok(())
    }
}

/// Generic trait to make it possible to implement common code for both TCP and UNIX streams
pub trait ReadWrite: Read + Write {}

impl<T: Read + Write> ReadWrite for T {}

/// Default sleep time in milliseconds for the server threads.
const SLEEP_DELAY_MS: u64 = 200;

/// Runs the server, requiring arguments of `TYPE` `PORT` `PASSWORD`.
/// Type can be `TCP` or `UNIX`
/// Port depends on the type (either `8080` for TCP, or `/tmp/luxo_server` named pipe recommended)
/// Password must be a [`String`].
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    // We check the length of arguments to ensure they are ok
    if args.len() != 4 {
        return Err(
            "incorrect number of arguments provided. use `server TYPE PORT PASSWORD` where TYPE = TCP|UNIX"
                .to_string(),
        );
    }
    // If we enter something else, we return an error to the user
    let server_type = match args[1].as_str() {
        "TCP" => ServerType::TCP,
        "UNIX" => ServerType::UNIX,
        _ => return Err("invalid server type given (choose UNIX or TCP)".to_string()),
    };

    let port = args[2].as_str();
    let password = args[3].clone();

    let server_data = ServerData::new(password, server_type);

    // Run the server
    server_process::run_server(server_data, port).unwrap();

    Ok(())
}
