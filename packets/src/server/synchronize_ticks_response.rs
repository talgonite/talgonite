use crate::TryFromBytes;
use anyhow::anyhow;

#[derive(Debug)]
pub struct SynchronizeTicksResponse {
    pub ticks: i32,
}

impl TryFromBytes for SynchronizeTicksResponse {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let slice = bytes
            .get(0..4)
            .ok_or_else(|| anyhow!("SynchronizeTicksResponse packet too short"))?;
        let arr: [u8; 4] = slice
            .try_into()
            .map_err(|_| anyhow!("SynchronizeTicksResponse packet wrong length"))?;
        Ok(SynchronizeTicksResponse {
            ticks: i32::from_be_bytes(arr),
        })
    }
}
