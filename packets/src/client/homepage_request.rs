use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct HomepageRequest;

impl ToBytes for HomepageRequest {
    const OPCODE: u8 = Codes::HomepageRequest as _;

    fn write_payload(&self, _bytes: &mut Vec<u8>) {}
}
