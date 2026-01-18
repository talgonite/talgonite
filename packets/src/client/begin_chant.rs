use crate::{FromBytes, ToBytes};

use super::Codes;

#[derive(Debug)]
pub struct BeginChant {
    pub cast_line_count: u8,
}

impl ToBytes for BeginChant {
    const OPCODE: u8 = Codes::BeginChant as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.cast_line_count);
    }
}

impl FromBytes for BeginChant {
    fn from_bytes(bytes: &[u8]) -> Self {
        BeginChant {
            cast_line_count: bytes[0],
        }
    }
}
