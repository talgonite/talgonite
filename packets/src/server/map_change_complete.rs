use crate::TryFromBytes;

#[derive(Debug)]
pub struct MapChangeComplete {}

impl TryFromBytes for MapChangeComplete {
    fn try_from_bytes(_bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(MapChangeComplete {})
    }
}
