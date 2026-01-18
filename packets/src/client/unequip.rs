use crate::{ToBytes, types::EquipmentSlot};

use super::Codes;

#[derive(Debug)]
pub struct Unequip {
    pub equipment_slot: EquipmentSlot,
}

impl ToBytes for Unequip {
    const OPCODE: u8 = Codes::Unequip as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        let slot: u8 = self.equipment_slot.into();
        bytes.push(slot);
    }
}
