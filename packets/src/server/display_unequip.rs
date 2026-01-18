use crate::{TryFromBytes, types::EquipmentSlot};
use anyhow::anyhow;

#[derive(Debug, Clone)]
pub struct DisplayUnequip {
    pub equipment_slot: EquipmentSlot,
}

impl TryFromBytes for DisplayUnequip {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let equipment_slot = bytes
            .get(0)
            .copied()
            .ok_or_else(|| anyhow!("DisplayUnequip packet too short"))?
            .try_into()
            .map_err(|_| anyhow!("Invalid equipment slot in DisplayUnequip packet"))?;
        Ok(DisplayUnequip { equipment_slot })
    }
}
