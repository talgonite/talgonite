use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::ReadBytesExt;
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use std::io::{Cursor, Read};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginMessageType {
    /// <summary>
    ///     A generic confirmation window with an ok button
    /// </summary>
    Confirm,

    /// <summary>
    ///     Clears the name field during character creation and presents a message with an ok button
    /// </summary>
    ClearNameMessage,

    /// <summary>
    ///     Indicates the requested character name already exists or contains a reserved string
    /// </summary>
    NameExistsOrReserved,

    /// <summary>
    ///     Clears the password field during character creation and presents a message with an ok button
    /// </summary>
    ClearPswdMessage,

    /// <summary>
    ///     Clears the name and password fields on the login screen and presents a message with an ok button
    /// </summary>
    CharacterDoesntExist,

    /// <summary>
    ///     Clears the password fields on the login screen and presents a message with an ok button
    /// </summary>
    WrongPassword,

    Other(u8),
}

impl LoginMessageType {
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Confirm,
            3 => Self::ClearNameMessage,
            4 => Self::NameExistsOrReserved,
            5 => Self::ClearPswdMessage,
            14 => Self::CharacterDoesntExist,
            15 => Self::WrongPassword,
            other => Self::Other(other),
        }
    }

    pub fn code(self) -> u8 {
        match self {
            Self::Confirm => 0,
            Self::ClearNameMessage => 3,
            Self::NameExistsOrReserved => 4,
            Self::ClearPswdMessage => 5,
            Self::CharacterDoesntExist => 14,
            Self::WrongPassword => 15,
            Self::Other(code) => code,
        }
    }
}

#[derive(Debug)]
pub struct LoginMessage {
    pub msg_type: LoginMessageType,
    pub msg: String,
}

impl TryFromBytes for LoginMessage {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let msg_type = LoginMessageType::from_u8(cursor.read_u8()?);
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
