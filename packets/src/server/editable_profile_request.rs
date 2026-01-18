use crate::TryFromBytes;

#[derive(Debug)]
pub struct EditableProfileRequest;

impl TryFromBytes for EditableProfileRequest {
    fn try_from_bytes(_bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(Self)
    }
}
