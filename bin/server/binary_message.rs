use std::mem::size_of_val;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum MessageType {
    Unknown,
    Command,
    Message,
}

#[derive(Debug, Clone)]
/// Message struct to communicate between server and client
pub struct BinaryMessage {
    length: u16,
    message_type: MessageType,
    message: Vec<u8>,
}

impl Default for BinaryMessage {
    fn default() -> Self {
        Self {
            length: Default::default(),
            message_type: MessageType::Unknown,
            message: Default::default(),
        }
    }
}

impl BinaryMessage {
    /// Creates a new [`BinaryMessage`] message
    pub fn new_message(text: String) -> Self {
        Self {
            length: text.len() as u16,
            message_type: MessageType::Message,
            message: text.into_bytes(),
        }
    }

    /// Creates a new [`BinaryMessage`] command.
    pub fn new_command(text: String) -> Self {
        Self {
            length: text.len() as u16,
            message_type: MessageType::Command,
            message: text.into_bytes(),
        }
    }

    /// Geths the type of the message
    pub fn get_type(&self) -> MessageType {
        self.message_type
    }

    /// Geths the message
    pub fn get_message(&self) -> &Vec<u8> {
        &self.message
    }

    /// Serializes the data into bytes
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::default();

        bytes.extend(self.length.to_be_bytes());
        bytes.push(self.message_type as u8);
        bytes.extend(self.message.clone());
        bytes
    }

    /// Deserializes the bytes into a [`BinaryMessage`]
    pub fn deserialize(message: &[u8]) -> Self {
        let mut bin_message = BinaryMessage::default();

        let required = size_of_val(&bin_message.length) + size_of_val(&bin_message.message_type);

        if message.len() < required {
            return Self {
                length: 0,
                message_type: MessageType::Unknown,
                message: Vec::default(),
            };
        }

        // Length is 2 bytes
        bin_message.length = u16::from_be_bytes([message[0], message[1]]);

        // Match the message type
        bin_message.message_type = match message[2] {
            1 => MessageType::Command,
            2 => MessageType::Message,
            _ => MessageType::Unknown,
        };

        if bin_message.length > 0 {
            // The message is from the 4th byte to the length of the message, to prevent an exploit
            bin_message.message = message[3..bin_message.length as usize + 3].to_vec();
        } else {
            bin_message.message = Vec::default();
        }

        bin_message
    }

    /// Function to split the message into a command and arguments
    pub fn split(&self) -> Result<(String, Vec<u8>), std::string::FromUtf8Error> {
        let space_index = self.message.iter().position(|&x| x == 32);

        match space_index {
            Some(space_index) => {
                let command = String::from_utf8(self.message[0..space_index].to_vec())?;
                let arguments = if self.length > (space_index as u16 + 1) {
                    self.message[(space_index + 1)..self.length as usize].to_vec()
                } else {
                    vec![]
                };
                Ok((command, arguments))
            }
            None => {
                let command = String::from_utf8(self.message.clone())?;
                Ok((command, vec![]))
            }
        }
    }
}
