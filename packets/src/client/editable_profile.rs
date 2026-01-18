use crate::ToBytes;

use super::Codes;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};

#[derive(Debug)]
pub struct EditableProfile {
    pub portrait_data: Vec<u8>,
    pub profile_message: String,
}

impl ToBytes for EditableProfile {
    const OPCODE: u8 = Codes::EditableProfile as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(
            &((self.portrait_data.len() + self.profile_message.len()) as u16 + 4).to_be_bytes(),
        );
        bytes.extend_from_slice(&(self.portrait_data.len() as u16).to_be_bytes());
        bytes.extend_from_slice(&self.portrait_data);
        let profile_message_bytes = WINDOWS_949
            .encode(&self.profile_message, EncoderTrap::Replace)
            .unwrap_or_default();
        bytes.extend_from_slice(&(profile_message_bytes.len() as u16).to_be_bytes());
        bytes.extend_from_slice(&profile_message_bytes);
    }
}
