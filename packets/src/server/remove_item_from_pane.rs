use crate::TryFromBytes;
use anyhow::anyhow;

#[derive(Debug, Clone)]
pub struct RemoveItemFromPane {
    pub slot: u8,
}

impl TryFromBytes for RemoveItemFromPane {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let slot = bytes.get(0).copied().unwrap_or_default();

        if slot == 0 {
            return Err(anyhow!("Invalid slot number: {}", slot));
        }

        Ok(RemoveItemFromPane { slot })
    }
}
