use crate::ToBytes;
use num_enum::IntoPrimitive;

use super::Codes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
#[repr(u8)]
pub enum SwapSlotPanelType {
    Inventory = 0,
    Spell = 1,
    Skill = 2,
    Equipment = 3,
}

#[derive(Debug)]
pub struct SwapSlot {
    pub panel_type: SwapSlotPanelType,
    pub slot1: u8,
    pub slot2: u8,
}

impl ToBytes for SwapSlot {
    const OPCODE: u8 = Codes::SwapSlot as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.panel_type.into());
        bytes.push(self.slot1);
        bytes.push(self.slot2);
    }
}
