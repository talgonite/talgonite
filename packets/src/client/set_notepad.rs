use super::Codes;
use crate::ToBytes;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};

#[derive(Debug)]
pub struct SetNotepad {
    pub slot: u8,
    pub message: String,
}

impl ToBytes for SetNotepad {
    const OPCODE: u8 = Codes::SetNotepad as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        let enc_bytes = WINDOWS_949
            .encode(&self.message, EncoderTrap::Replace)
            .unwrap_or_default();

        bytes.push(self.slot);
        bytes.extend_from_slice(&(enc_bytes.len() as u16).to_be_bytes());
        bytes.extend_from_slice(&enc_bytes);
    }
}
