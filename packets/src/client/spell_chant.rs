use crate::ToBytes;

use super::Codes;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};

#[derive(Debug)]
pub struct SpellChant {
    pub chant_message: String,
}

impl ToBytes for SpellChant {
    const OPCODE: u8 = Codes::Chant as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        let chant_bytes = WINDOWS_949
            .encode(&self.chant_message, EncoderTrap::Replace)
            .unwrap_or_default();
        bytes.push(chant_bytes.len() as u8);
        bytes.extend_from_slice(&chant_bytes);
    }
}
