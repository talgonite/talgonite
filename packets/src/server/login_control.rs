use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::ReadBytesExt;
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::io::Read;

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum LoginControlsType {
    /// <summary>
    ///     Tells the client that the packet contains the homepage url
    /// </summary>
    Homepage = 3,
}

#[derive(Debug)]
pub struct LoginControl {
    pub login_controls_type: LoginControlsType,
    pub message: String,
}

impl TryFromBytes for LoginControl {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = std::io::Cursor::new(bytes);
        let login_controls_type = cursor.read_u8()?.try_into()?;
        let message = {
            let mut buf = vec![0; cursor.read_u8()? as usize];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode message: {}", e))?
        };
        Ok(LoginControl {
            login_controls_type,
            message,
        })
    }
}
