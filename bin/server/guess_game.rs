use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
/// A game has many tracked parameters.
pub struct Game {
    game_id: u128,
    host_id: u64,
    opponent_id: u64,
    secret: String,
    attempts: u8,
    last_guess: String,
    last_hint: String,
    game_state: GameState,
}

#[derive(Debug, Clone)]
/// The state in which a game is in.
pub enum GameState {
    Victory,
    Defeat,
    Ongoing,
}

impl Game {
    /// Creates a new [`Game`].
    pub fn new(id_host: u64, id_guest: u64, secret: String) -> Self {
        // We get the time + IDs of both players to ensure a unique ID
        let start = SystemTime::now();
        let since_the_epoch = start
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards!!!");
        let timestamp =
            since_the_epoch.as_secs() * 1_000_000_000 + since_the_epoch.subsec_nanos() as u64;
        let game_id = format!("{timestamp}{id_guest}{id_host}");
        // We really shouldnt fail here at all, so it should be fine
        let game_id = game_id.parse().unwrap();

        Self {
            game_id,
            host_id: id_host,
            opponent_id: id_guest,
            secret,
            attempts: 3,
            last_guess: String::new(),
            last_hint: String::new(),
            game_state: GameState::Ongoing,
        }
    }

    // There are all possible get/set methods for the struct,
    // just in case we'd want to expand the game in the future and needed them.

    /// Returns the unique identifier of the game.
    pub fn get_game_id(&self) -> u128 {
        self.game_id
    }

    /// Returns the host_id from a game
    pub fn get_host_id(&self) -> u64 {
        self.host_id
    }

    /// Returns the opponent_id from a game
    pub fn get_opponent_id(&self) -> u64 {
        self.opponent_id
    }

    /// Returns the secret from a game
    pub fn get_secret(&self) -> &String {
        &self.secret
    }

    /// Returns the attempts from a game
    pub fn get_attempts(&self) -> u8 {
        self.attempts
    }

    /// Returns the last guess from a game
    pub fn get_last_guess(&self) -> &String {
        &self.last_guess
    }

    /// Returns the last hint from a game
    pub fn get_last_hint(&self) -> &String {
        &self.last_hint
    }

    /// Returns the game state from a game
    pub fn get_game_state(&self) -> &GameState {
        &self.game_state
    }

    /// Sets the game game ID to a game ID
    pub fn set_game_id(&mut self, game_id: u128) {
        self.game_id = game_id;
    }

    /// Sets the host ID to a game
    pub fn set_host_id(&mut self, host_id: u64) {
        self.host_id = host_id;
    }

    /// Sets the opponent ID to a game
    pub fn set_opponent_id(&mut self, opponent_id: u64) {
        self.opponent_id = opponent_id;
    }

    /// Sets the secret to a game
    pub fn set_secret(&mut self, secret: String) {
        self.secret = secret;
    }

    /// Sets the attempts to a game
    pub fn set_attempts(&mut self, attempts: u8) {
        self.attempts = attempts;
    }

    /// Sets the last guess to a game
    pub fn set_last_guess(&mut self, last_guess: String) {
        self.last_guess = last_guess;
    }

    /// Sets the last hint to a game
    pub fn set_last_hint(&mut self, last_hint: String) {
        self.last_hint = last_hint;
    }

    /// Sets the game state to a game
    pub fn set_game_state(&mut self, game_state: GameState) {
        self.game_state = game_state;
    }
}
