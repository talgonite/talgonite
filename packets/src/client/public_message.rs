use crate::ToBytes;

use super::Codes;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};
use num_enum::IntoPrimitive;

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
#[repr(u8)]
pub enum PublicMessageType {
    Normal = 0,
    Shout = 1,
    Chant = 2,
}

#[derive(Debug)]
pub struct PublicMessage {
    pub public_message_type: PublicMessageType,
    pub message: String,
}

impl ToBytes for PublicMessage {
    const OPCODE: u8 = Codes::PublicMessage as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.public_message_type.into());
        let message_bytes = WINDOWS_949
            .encode(&self.message, EncoderTrap::Replace)
            .unwrap_or_default();
        bytes.push(message_bytes.len() as u8);
        bytes.extend_from_slice(&message_bytes);
    }
}
