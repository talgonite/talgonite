use crate::{ToBytes, types::Direction};

use super::Codes;

#[derive(Debug)]
pub struct ClientWalk {
    pub direction: Direction,
    pub step_count: u8,
}

impl ToBytes for ClientWalk {
    const OPCODE: u8 = Codes::ClientWalk as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.direction.into());
        bytes.push(self.step_count);
    }
}
