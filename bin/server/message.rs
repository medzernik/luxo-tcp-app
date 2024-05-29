use std::mem;

/// Main message struct to communicate between server and client
#[derive(Debug)]
pub struct Message {
    // This is basically a part of the server, so the split is not necessary
    pub args: String,
}

impl Message {
    /// Creates a new [`Message`].
    pub fn new(data: &str) -> Self {
        Self {
            args: data.to_string(),
        }
    }

    /// Serializes the [`Message`] to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::default();
        let length = self.args.len() as u16;
        bytes.extend(&length.to_be_bytes());
        bytes.push(1);
        bytes.extend(self.args.as_bytes());
        bytes
    }

    /// Deserializes the message from bytes into the [`Message`]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < mem::size_of::<u32>() {
            eprintln!("error deserializing: not enough bytes for length");
            return None;
        }
        let length = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        if bytes.len() < mem::size_of::<u32>() + length {
            eprintln!("error deserializing: not enough bytes for data");
            return None;
        }
        match String::from_utf8(
            bytes[mem::size_of::<u32>()..mem::size_of::<u32>() + length].to_vec(),
        ) {
            Ok(value) => Some(Self { args: value }),
            Err(_) => {
                eprintln!("error deserializing: invalid UTF-8");
                None
            }
        }
    }
}
