use crate::TryFromBytes;

#[derive(Debug)]
pub struct CancelCasting;

impl TryFromBytes for CancelCasting {
    fn try_from_bytes(_bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(CancelCasting {})
    }
}
