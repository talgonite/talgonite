use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct HeartBeat {
    pub first: u8,
    pub second: u8,
}

impl ToBytes for HeartBeat {
    const OPCODE: u8 = Codes::HeartBeat as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.first);
        bytes.push(self.second);
    }
}
