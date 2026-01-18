use crate::TryFromBytes;
use anyhow::anyhow;

#[derive(Debug, Clone)]
pub struct RemoveSkillFromPane {
    pub slot: u8,
}

impl TryFromBytes for RemoveSkillFromPane {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let slot = bytes
            .get(0)
            .copied()
            .ok_or_else(|| anyhow!("RemoveSkillFromPane packet too short"))?;
        Ok(RemoveSkillFromPane { slot })
    }
}
