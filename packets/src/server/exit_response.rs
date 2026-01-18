use crate::TryFromBytes;
use byteorder::ReadBytesExt;

#[derive(Debug, Clone)]
pub struct ExitResponse {
    pub exit_confirmed: bool,
}

impl TryFromBytes for ExitResponse {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = std::io::Cursor::new(bytes);
        let exit_confirmed = cursor.read_u8()? != 0;
        Ok(Self { exit_confirmed })
    }
}
