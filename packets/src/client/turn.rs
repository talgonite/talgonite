use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct Turn {
    pub direction: u8,
}

impl ToBytes for Turn {
    const OPCODE: u8 = Codes::Turn as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.direction);
    }
}
