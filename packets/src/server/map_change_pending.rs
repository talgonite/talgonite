use crate::TryFromBytes;

#[derive(Debug, Clone, Default)]
pub struct MapChangePending;

impl TryFromBytes for MapChangePending {
    fn try_from_bytes(_bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(Self)
    }
}
