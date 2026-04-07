//! Opcode 47 (0x2F). No payload.
//! Used for: toggling "accepting invites" and for leaving the current group (leader or not).

use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct ToggleGroup;

impl ToBytes for ToggleGroup {
    const OPCODE: u8 = Codes::ToggleGroup as _;

    fn write_payload(&self, _bytes: &mut Vec<u8>) {}
}
