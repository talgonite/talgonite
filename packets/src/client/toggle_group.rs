use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct ToggleGroup;

impl ToBytes for ToggleGroup {
    const OPCODE: u8 = Codes::ToggleGroup as _;

    fn write_payload(&self, _bytes: &mut Vec<u8>) {}
}
