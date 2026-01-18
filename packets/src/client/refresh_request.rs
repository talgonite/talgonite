use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct RefreshRequest;

impl ToBytes for RefreshRequest {
    const OPCODE: u8 = Codes::RefreshRequest as _;

    fn write_payload(&self, _bytes: &mut Vec<u8>) {}
}
