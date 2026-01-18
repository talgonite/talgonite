use crate::TryFromBytes;
use crate::types::{EntityType, MenuType};
use anyhow::anyhow;
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use std::io::{Cursor, Read};

const ITEM_SPRITE_OFFSET: u16 = 0x8000;

#[derive(Debug, Clone)]
pub struct ItemInfo {
    pub sprite: u16,
    pub color: u8,
    pub cost: i32,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct SpellInfo {
    pub sprite: u16,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct SkillInfo {
    pub sprite: u16,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct DisplayMenuHeader {
    pub entity_type: EntityType,
    pub source_id: u32,
    pub sprite: u16,
    pub color: u8,
    pub should_illustrate: bool,
    pub name: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub enum DisplayMenuPayload {
    Menu {
        options: Vec<(String, u16)>,
    },
    MenuWithArgs {
        args: String,
        options: Vec<(String, u16)>,
    },
    TextEntry {
        pursuit_id: u16,
    },
    TextEntryWithArgs {
        args: String,
        pursuit_id: u16,
    },
    ShowItems {
        pursuit_id: u16,
        items: Vec<ItemInfo>,
    },
    ShowPlayerItems {
        pursuit_id: u16,
        slots: Vec<u8>,
    },
    ShowSpells {
        pursuit_id: u16,
        spells: Vec<SpellInfo>,
    },
    ShowSkills {
        pursuit_id: u16,
        skills: Vec<SkillInfo>,
    },
    ShowPlayerSpells {
        pursuit_id: u16,
    },
    ShowPlayerSkills {
        pursuit_id: u16,
    },
}

#[derive(Debug, Clone)]
pub struct DisplayMenu {
    pub menu_type: MenuType,
    pub header: DisplayMenuHeader,
    pub payload: DisplayMenuPayload,
}

impl TryFromBytes for DisplayMenu {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let menu_type = MenuType::try_from(cursor.read_u8()?)?;
        let entity_type = EntityType::try_from(cursor.read_u8()?)?;
        let source_id = cursor.read_u32::<BigEndian>()?;
        cursor.read_u8()?; // unknown
        let sprite = cursor.read_u16::<BigEndian>()?;
        let color = cursor.read_u8()?;
        cursor.read_u8()?; // unknown
        let sprite2 = cursor.read_u16::<BigEndian>()?;
        let color2 = cursor.read_u8()?;
        let should_illustrate = cursor.read_u8()? != 0;

        let decode_with_len =
            |cursor: &mut Cursor<&[u8]>, len: usize, label: &str| -> anyhow::Result<String> {
                let mut buf = vec![0; len];
                cursor.read_exact(&mut buf)?;
                WINDOWS_949
                    .decode(&buf, DecoderTrap::Replace)
                    .map_err(|e| anyhow!("Failed to decode {}: {}", label, e))
            };

        let decode_string_u8 =
            |cursor: &mut Cursor<&[u8]>, label: &str| -> anyhow::Result<String> {
                let len = cursor.read_u8()? as usize;
                if len == 0 {
                    return Ok(String::new());
                }
                decode_with_len(cursor, len, label)
            };

        let decode_string_u16 =
            |cursor: &mut Cursor<&[u8]>, label: &str| -> anyhow::Result<String> {
                let len = cursor.read_u16::<BigEndian>()? as usize;
                if len == 0 {
                    return Ok(String::new());
                }
                decode_with_len(cursor, len, label)
            };

        let name = decode_string_u8(&mut cursor, "name")?;
        let text = decode_string_u16(&mut cursor, "text")?;

        let header = DisplayMenuHeader {
            entity_type,
            source_id,
            sprite: if sprite == 0 { sprite2 } else { sprite },
            color: if color == 0 { color2 } else { color },
            should_illustrate,
            name,
            text,
        };

        let decode_options = |cursor: &mut Cursor<&[u8]>| -> anyhow::Result<Vec<(String, u16)>> {
            let count = cursor.read_u8()?;
            let mut options = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let text = decode_string_u8(cursor, "option text")?;
                let pursuit = cursor.read_u16::<BigEndian>()?;
                options.push((text, pursuit));
            }
            Ok(options)
        };

        let payload = match menu_type {
            MenuType::Menu => DisplayMenuPayload::Menu {
                options: decode_options(&mut cursor)?,
            },
            MenuType::MenuWithArgs => DisplayMenuPayload::MenuWithArgs {
                args: decode_string_u8(&mut cursor, "args")?,
                options: decode_options(&mut cursor)?,
            },
            MenuType::TextEntry => DisplayMenuPayload::TextEntry {
                pursuit_id: cursor.read_u16::<BigEndian>()?,
            },
            MenuType::TextEntryWithArgs => DisplayMenuPayload::TextEntryWithArgs {
                args: decode_string_u8(&mut cursor, "args")?,
                pursuit_id: cursor.read_u16::<BigEndian>()?,
            },
            MenuType::ShowItems => {
                let pursuit_id = cursor.read_u16::<BigEndian>()?;
                let count = cursor.read_u16::<BigEndian>()?;
                let mut items = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let sprite = cursor
                        .read_u16::<BigEndian>()?
                        .saturating_sub(ITEM_SPRITE_OFFSET);
                    let color = cursor.read_u8()?;
                    let cost = cursor.read_u32::<BigEndian>()? as i32;
                    let name = decode_string_u8(&mut cursor, "item name")?;
                    let meta_len = cursor.read_u8()?;
                    let pos = cursor.position();
                    cursor.set_position(pos + meta_len as u64);
                    items.push(ItemInfo {
                        sprite,
                        color,
                        cost,
                        name,
                    });
                }
                DisplayMenuPayload::ShowItems { pursuit_id, items }
            }
            MenuType::ShowPlayerItems => {
                let pursuit_id = cursor.read_u16::<BigEndian>()?;
                let count = cursor.read_u8()?;
                let mut slots = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    slots.push(cursor.read_u8()?);
                }
                DisplayMenuPayload::ShowPlayerItems { pursuit_id, slots }
            }
            MenuType::ShowSpells => {
                let pursuit_id = cursor.read_u16::<BigEndian>()?;
                let count = cursor.read_u16::<BigEndian>()?;
                let mut spells = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    cursor.read_u8()?;
                    let sprite = cursor.read_u16::<BigEndian>()?;
                    cursor.read_u8()?;
                    let name = decode_string_u8(&mut cursor, "spell name")?;
                    spells.push(SpellInfo { sprite, name });
                }
                DisplayMenuPayload::ShowSpells { pursuit_id, spells }
            }
            MenuType::ShowSkills => {
                let pursuit_id = cursor.read_u16::<BigEndian>()?;
                let count = cursor.read_u16::<BigEndian>()?;
                let mut skills = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    cursor.read_u8()?;
                    let sprite = cursor.read_u16::<BigEndian>()?;
                    cursor.read_u8()?;
                    let name = decode_string_u8(&mut cursor, "skill name")?;
                    skills.push(SkillInfo { sprite, name });
                }
                DisplayMenuPayload::ShowSkills { pursuit_id, skills }
            }
            MenuType::ShowPlayerSpells => DisplayMenuPayload::ShowPlayerSpells {
                pursuit_id: cursor.read_u16::<BigEndian>()?,
            },
            MenuType::ShowPlayerSkills => DisplayMenuPayload::ShowPlayerSkills {
                pursuit_id: cursor.read_u16::<BigEndian>()?,
            },
        };

        Ok(DisplayMenu {
            menu_type,
            header,
            payload,
        })
    }
}
