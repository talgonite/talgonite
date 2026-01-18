use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::ReadBytesExt;
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::io::{Cursor, Read};

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum LoginMessageType {
    /// <summary>
    ///     A generic confirmation window with an ok button
    /// </summary>
    Confirm = 0,

    /// <summary>
    ///     Clears the name field during character creation and presents a message with an ok button
    /// </summary>
    ClearNameMessage = 3,

    /// <summary>
    ///     Clears the password field during character creation and presents a message with an ok button
    /// </summary>
    ClearPswdMessage = 5,

    /// <summary>
    ///     Clears the name and password fields on the login screen and presents a message with an ok button
    /// </summary>
    CharacterDoesntExist = 14,

    /// <summary>
    ///     Clears the password fields on the login screen and presents a message with an ok button
    /// </summary>
    WrongPassword = 15,
}

#[derive(Debug)]
pub struct LoginMessage {
    pub msg_type: LoginMessageType,
    pub msg: String,
}

impl TryFromBytes for LoginMessage {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let msg_type = cursor.read_u8()?.try_into()?;
        let msg = {
            let mut buf = vec![0; cursor.read_u8()? as usize];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode msg: {}", e))?
        };
        Ok(LoginMessage { msg_type, msg })
    }
}
