use crate::TryFromBytes;
use byteorder::ReadBytesExt;

#[derive(Debug)]
pub struct HeartBeatResponse {
    pub first: u8,
    pub second: u8,
}

impl TryFromBytes for HeartBeatResponse {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = std::io::Cursor::new(bytes);
        let first = cursor.read_u8()?;
        let second = cursor.read_u8()?;
        Ok(HeartBeatResponse { first, second })
    }
}
