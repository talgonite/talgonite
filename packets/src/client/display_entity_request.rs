use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct DisplayEntityRequest {
    pub target_id: u32,
}

impl ToBytes for DisplayEntityRequest {
    const OPCODE: u8 = Codes::DisplayEntityRequest as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.target_id.to_be_bytes());
    }
}
