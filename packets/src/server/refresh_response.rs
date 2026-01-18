use crate::TryFromBytes;

#[derive(Debug)]
pub struct RefreshResponse;

impl TryFromBytes for RefreshResponse {
    fn try_from_bytes(_bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(RefreshResponse {})
    }
}
