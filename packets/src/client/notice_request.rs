use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct NoticeRequest;

impl ToBytes for NoticeRequest {
    const OPCODE: u8 = Codes::NoticeRequest as _;

    fn write_payload(&self, _bytes: &mut Vec<u8>) {}
}
