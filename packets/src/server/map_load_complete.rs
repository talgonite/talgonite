use crate::TryFromBytes;

#[derive(Debug)]
pub struct MapLoadComplete;

impl TryFromBytes for MapLoadComplete {
    fn try_from_bytes(_bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(Self)
    }
}
