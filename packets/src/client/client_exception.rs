use crate::ToBytes;

use super::Codes;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};

#[derive(Debug)]
pub struct ClientException {
    pub exception_str: String,
}

impl ToBytes for ClientException {
    const OPCODE: u8 = Codes::ClientException as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        let exception_bytes = WINDOWS_949
            .encode(&self.exception_str, EncoderTrap::Replace)
            .unwrap_or_default();
        bytes.push(exception_bytes.len() as u8);
        bytes.extend_from_slice(&exception_bytes);
    }
}
