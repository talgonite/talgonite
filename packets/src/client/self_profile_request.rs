use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct SelfProfileRequest;

impl ToBytes for SelfProfileRequest {
    const OPCODE: u8 = Codes::SelfProfileRequest as _;

    fn write_payload(&self, _bytes: &mut Vec<u8>) {}
}
