use crate::ToBytes;

use super::Codes;

#[derive(Debug)]
pub struct SkillUse {
    pub source_slot: u8,
}

impl ToBytes for SkillUse {
    const OPCODE: u8 = Codes::SkillUse as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.source_slot);
    }
}
