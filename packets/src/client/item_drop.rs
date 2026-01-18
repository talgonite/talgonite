use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct ItemDrop {
    pub source_slot: u8,
    pub destination_point: (u16, u16),
    pub count: i32,
}

impl ToBytes for ItemDrop {
    const OPCODE: u8 = Codes::ItemDrop as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.source_slot);
        bytes.extend_from_slice(&self.destination_point.0.to_be_bytes());
        bytes.extend_from_slice(&self.destination_point.1.to_be_bytes());
        bytes.extend_from_slice(&self.count.to_be_bytes());
    }
}
