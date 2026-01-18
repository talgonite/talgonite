use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct MapDataRequest {
    pub x: u8,
    pub y: u8,
    pub checksum: [u8; 3],
}

impl ToBytes for MapDataRequest {
    const OPCODE: u8 = Codes::MapDataRequest as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(0);
        bytes.push(0);
        bytes.push(0);
        bytes.push(0);
        bytes.push(self.x);
        bytes.push(self.y);
        bytes.extend_from_slice(&self.checksum);
    }
}
