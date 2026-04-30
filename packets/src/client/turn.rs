use crate::{ToBytes, types::Direction};

use super::Codes;

#[derive(Debug)]
pub struct Turn {
    pub direction: Direction,
}

impl ToBytes for Turn {
    const OPCODE: u8 = Codes::Turn as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.direction.into());
    }
}
