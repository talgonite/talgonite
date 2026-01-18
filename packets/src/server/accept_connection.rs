use crate::TryFromBytes;

#[derive(Debug)]
pub struct AcceptConnection;

impl TryFromBytes for AcceptConnection {
    fn try_from_bytes(_bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(AcceptConnection)
    }
}
