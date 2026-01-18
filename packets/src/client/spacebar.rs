use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct Spacebar;

impl ToBytes for Spacebar {
    const OPCODE: u8 = Codes::Spacebar as _;

    fn write_payload(&self, _bytes: &mut Vec<u8>) {}
}
