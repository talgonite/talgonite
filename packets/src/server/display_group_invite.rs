use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::ReadBytesExt;
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use num_enum::TryFromPrimitive;
use std::io::{Cursor, Read};

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum ServerGroupSwitch {
    Invite = 0,
    ShowGroupBox = 1,
}

#[derive(Debug, Clone)]
pub struct DisplayGroupBoxInfo {
    pub name: String,
    pub note: String,
    pub min_level: u8,
    pub max_level: u8,
    pub max_warriors: u8,
    pub current_warriors: u8,
    pub max_wizards: u8,
    pub current_wizards: u8,
    pub max_monks: u8,
    pub current_monks: u8,
    pub max_priests: u8,
    pub current_priests: u8,
    pub max_rogues: u8,
    pub current_rogues: u8,
}

#[derive(Debug, Clone)]
pub enum DisplayGroupInvite {
    Invite {
        source_name: String,
        group_box_info: DisplayGroupBoxInfo,
    },
    ShowGroupBox {
        source_name: String,
    },
}

impl TryFromBytes for DisplayGroupInvite {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let switch_byte = cursor.read_u8()?;
        let server_group_switch = switch_byte
            .try_into()
            .map_err(|_| anyhow!("Invalid server group switch type: {}", switch_byte))?;

        let decode_string = |cursor: &mut Cursor<&[u8]>, label: &str| -> anyhow::Result<String> {
            let len = cursor.read_u8()? as usize;
            let mut buf = vec![0; len];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode {}: {}", label, e))
        };

        let source_name = decode_string(&mut cursor, "source_name")?;

        match server_group_switch {
            ServerGroupSwitch::Invite => {
                let name = decode_string(&mut cursor, "group name")?;
                let note = decode_string(&mut cursor, "group note")?;
                Ok(DisplayGroupInvite::Invite {
                    source_name,
                    group_box_info: DisplayGroupBoxInfo {
                        name,
                        note,
                        min_level: cursor.read_u8()?,
                        max_level: cursor.read_u8()?,
                        max_warriors: cursor.read_u8()?,
                        current_warriors: cursor.read_u8()?,
                        max_wizards: cursor.read_u8()?,
                        current_wizards: cursor.read_u8()?,
                        max_monks: cursor.read_u8()?,
                        current_monks: cursor.read_u8()?,
                        max_priests: cursor.read_u8()?,
                        current_priests: cursor.read_u8()?,
                        max_rogues: cursor.read_u8()?,
                        current_rogues: cursor.read_u8()?,
                    },
                })
            }
            ServerGroupSwitch::ShowGroupBox => Ok(DisplayGroupInvite::ShowGroupBox { source_name }),
        }
    }
}
