use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct Emote {
    pub animation: u8,
}

impl ToBytes for Emote {
    const OPCODE: u8 = Codes::Emote as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.animation);
    }
}
