use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct GoldDrop {
    pub amount: i32,
    pub destination_point: (u16, u16),
}

impl ToBytes for GoldDrop {
    const OPCODE: u8 = Codes::GoldDrop as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.amount.to_be_bytes());
        bytes.extend_from_slice(&self.destination_point.0.to_be_bytes());
        bytes.extend_from_slice(&self.destination_point.1.to_be_bytes());
    }
}
