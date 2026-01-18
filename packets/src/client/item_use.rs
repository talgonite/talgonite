use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct ItemUse {
    pub source_slot: u8,
}

impl ToBytes for ItemUse {
    const OPCODE: u8 = Codes::ItemUse as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.source_slot);
    }
}
