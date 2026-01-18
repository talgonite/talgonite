use crate::ToBytes;

use super::Codes;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};

#[derive(Debug)]
pub struct Whisper {
    pub target_name: String,
    pub message: String,
}

impl ToBytes for Whisper {
    const OPCODE: u8 = Codes::Whisper as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        let target_name_bytes = WINDOWS_949
            .encode(&self.target_name, EncoderTrap::Replace)
            .unwrap_or_default();
        bytes.push(target_name_bytes.len() as u8);
        bytes.extend_from_slice(&target_name_bytes);

        let message_bytes = WINDOWS_949
            .encode(&self.message, EncoderTrap::Replace)
            .unwrap_or_default();
        bytes.push(message_bytes.len() as u8);
        bytes.extend_from_slice(&message_bytes);
    }
}
