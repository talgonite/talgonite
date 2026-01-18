use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct ExitRequest {
    pub is_request: bool,
}

impl ToBytes for ExitRequest {
    const OPCODE: u8 = Codes::ExitRequest as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.is_request as u8);
    }
}
