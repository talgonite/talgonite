use crate::TryFromBytes;
use anyhow::anyhow;

#[derive(Debug, Clone)]
pub struct RemoveEntity {
    pub source_id: u32,
}

impl TryFromBytes for RemoveEntity {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let slice = bytes
            .get(0..4)
            .ok_or_else(|| anyhow!("RemoveEntity packet too short"))?;
        let arr: [u8; 4] = slice
            .try_into()
            .map_err(|_| anyhow!("RemoveEntity packet wrong length"))?;
        Ok(RemoveEntity {
            source_id: u32::from_be_bytes(arr),
        })
    }
}
