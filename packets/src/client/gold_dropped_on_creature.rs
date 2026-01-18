use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct GoldDroppedOnCreature {
    pub amount: i32,
    pub target_id: u32,
}

impl ToBytes for GoldDroppedOnCreature {
    const OPCODE: u8 = Codes::GoldDroppedOnCreature as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.amount.to_be_bytes());
        bytes.extend_from_slice(&self.target_id.to_be_bytes());
    }
}
