use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct ClientWalk {
    pub direction: u8,
    pub step_count: u8,
}

impl ToBytes for ClientWalk {
    const OPCODE: u8 = Codes::ClientWalk as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.direction);
        bytes.push(self.step_count);
    }
}
