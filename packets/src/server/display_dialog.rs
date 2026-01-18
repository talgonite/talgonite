use crate::TryFromBytes;
use crate::types::EntityType;
use anyhow::anyhow;
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use num_enum::TryFromPrimitive;
use std::io::{Cursor, Read};

const ITEM_SPRITE_OFFSET: u16 = 0x8000;
const CREATURE_SPRITE_OFFSET: u16 = 0x4000;

fn strip_sprite_offset(value: u16) -> u16 {
    if value > ITEM_SPRITE_OFFSET {
        value - ITEM_SPRITE_OFFSET
    } else if value > CREATURE_SPRITE_OFFSET {
        value - CREATURE_SPRITE_OFFSET
    } else {
        value
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum DialogType {
    Normal = 0,
    DialogMenu = 2,
    TextEntry = 4,
    Speak = 5,
    CreatureMenu = 6,
    Protected = 9,
    CloseDialog = 10,
}

#[derive(Debug, Clone)]
pub struct DisplayDialogHeader {
    pub entity_type: EntityType,
    pub source_id: u32,
    pub sprite: u16,
    pub color: u8,
    pub pursuit_id: u16,
    pub dialog_id: u16,
    pub has_previous_button: bool,
    pub has_next_button: bool,
    pub should_illustrate: bool,
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct TextEntryInfo {
    pub prompt: String,
    pub length: u8,
}

#[derive(Debug, Clone)]
pub enum DisplayDialog {
    CloseDialog,
    Normal {
        header: DisplayDialogHeader,
    },
    DialogMenu {
        header: DisplayDialogHeader,
        options: Vec<String>,
    },
    TextEntry {
        header: DisplayDialogHeader,
        info: TextEntryInfo,
    },
    Speak {
        header: DisplayDialogHeader,
    },
    CreatureMenu {
        header: DisplayDialogHeader,
        options: Vec<String>,
    },
    Protected {
        header: DisplayDialogHeader,
    },
}

impl TryFromBytes for DisplayDialog {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let dialog_type_byte = cursor.read_u8()?;
        let dialog_type = dialog_type_byte
            .try_into()
            .map_err(|_| anyhow!("Invalid dialog type: {}", dialog_type_byte))?;

        if matches!(dialog_type, DialogType::CloseDialog) {
            return Ok(DisplayDialog::CloseDialog);
        }

        let entity_type_byte = cursor.read_u8()?;
        let entity_type = entity_type_byte
            .try_into()
            .map_err(|_| anyhow!("Invalid entity type: {}", entity_type_byte))?;

        let source_id = cursor.read_u32::<BigEndian>()?;
        cursor.read_u8()?; // dunno
        let sprite = cursor.read_u16::<BigEndian>()?;
        let color = cursor.read_u8()?;
        cursor.read_u8()?; // dunno
        let sprite2 = cursor.read_u16::<BigEndian>()?;
        let color2 = cursor.read_u8()?;
        let pursuit_id = cursor.read_u16::<BigEndian>()?;
        let dialog_id = cursor.read_u16::<BigEndian>()?;
        let has_previous_button = cursor.read_u8()? != 0;
        let has_next_button = cursor.read_u8()? != 0;
        let should_illustrate = cursor.read_u8()? == 0;

        let decode_string = |cursor: &mut Cursor<&[u8]>, label: &str| -> anyhow::Result<String> {
            let len = cursor.read_u8()? as usize;
            let mut buf = vec![0; len];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode {}: {}", label, e))
        };

        let name = decode_string(&mut cursor, "name")?;
        let text_len = cursor.read_u16::<BigEndian>()? as usize;
        let mut text_buf = vec![0; text_len];
        cursor.read_exact(&mut text_buf)?;
        let text = WINDOWS_949
            .decode(&text_buf, DecoderTrap::Replace)
            .map_err(|e| anyhow!("Failed to decode text: {}", e))?;

        let header = DisplayDialogHeader {
            entity_type,
            source_id,
            sprite: strip_sprite_offset(if sprite == 0 { sprite2 } else { sprite }),
            color: if color == 0 { color2 } else { color },
            pursuit_id,
            dialog_id,
            has_previous_button,
            has_next_button,
            should_illustrate,
            name,
            text,
        };

        let decode_options = |cursor: &mut Cursor<&[u8]>| -> anyhow::Result<Vec<String>> {
            let options_count = cursor.read_u8()?;
            let mut options = Vec::with_capacity(options_count as usize);
            for _ in 0..options_count {
                options.push(decode_string(cursor, "option")?);
            }
            Ok(options)
        };

        let payload = match dialog_type {
            DialogType::Normal => DisplayDialog::Normal { header },
            DialogType::DialogMenu => DisplayDialog::DialogMenu {
                header,
                options: decode_options(&mut cursor)?,
            },
            DialogType::TextEntry => DisplayDialog::TextEntry {
                header,
                info: TextEntryInfo {
                    prompt: decode_string(&mut cursor, "text entry prompt")?,
                    length: cursor.read_u8()?,
                },
            },
            DialogType::Speak => DisplayDialog::Speak { header },
            DialogType::CreatureMenu => DisplayDialog::CreatureMenu {
                header,
                options: decode_options(&mut cursor)?,
            },
            DialogType::Protected => DisplayDialog::Protected { header },
            DialogType::CloseDialog => unreachable!(),
        };

        Ok(payload)
    }
}
