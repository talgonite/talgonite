use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct Pickup {
    pub destination_slot: u8,
    pub source_point: (u16, u16),
}

impl ToBytes for Pickup {
    const OPCODE: u8 = Codes::Pickup as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.destination_slot);
        bytes.extend_from_slice(&self.source_point.0.to_be_bytes());
        bytes.extend_from_slice(&self.source_point.1.to_be_bytes());
    }
}
