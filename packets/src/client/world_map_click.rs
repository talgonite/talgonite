use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct WorldMapClick {
    pub check_sum: u16,
    pub map_id: u16,
    pub point: (u16, u16),
}

impl ToBytes for WorldMapClick {
    const OPCODE: u8 = Codes::WorldMapClick as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.check_sum.to_be_bytes());
        bytes.extend_from_slice(&self.map_id.to_be_bytes());
        bytes.extend_from_slice(&self.point.0.to_be_bytes());
        bytes.extend_from_slice(&self.point.1.to_be_bytes());
    }
}
