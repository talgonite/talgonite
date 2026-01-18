use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct WorldListRequest;

impl ToBytes for WorldListRequest {
    const OPCODE: u8 = Codes::WorldListRequest as _;

    fn write_payload(&self, _: &mut Vec<u8>) {}
}
