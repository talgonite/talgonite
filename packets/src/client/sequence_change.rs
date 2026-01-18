use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct SequenceChange;

impl ToBytes for SequenceChange {
    const OPCODE: u8 = Codes::SequenceChange as _;

    fn write_payload(&self, _bytes: &mut Vec<u8>) {}
}
