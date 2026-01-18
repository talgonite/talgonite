use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct ItemDroppedOnCreature {
    pub source_slot: u8,
    pub target_id: u32,
    pub count: u8,
}

impl ToBytes for ItemDroppedOnCreature {
    const OPCODE: u8 = Codes::ItemDroppedOnCreature as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.source_slot);
        bytes.extend_from_slice(&self.target_id.to_be_bytes());
        bytes.push(self.count);
    }
}
