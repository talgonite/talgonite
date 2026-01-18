use crate::TryFromBytes;
use anyhow::anyhow;

#[derive(Debug, Clone)]
pub struct RemoveSpellFromPane {
    pub slot: u8,
}

impl TryFromBytes for RemoveSpellFromPane {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let slot = bytes
            .get(0)
            .copied()
            .ok_or_else(|| anyhow!("RemoveSpellFromPane packet too short"))?;
        Ok(RemoveSpellFromPane { slot })
    }
}
