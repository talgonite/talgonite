use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use std::io::{Cursor, Read};

const CREATURE_SPRITE_OFFSET: u16 = 0x8000; // matches server NETWORKING_CONSTANTS.CREATURE_SPRITE_OFFSET

#[derive(Debug, Clone)]
pub enum DisplayArgs {
    /// BodySprite.None + transparent flag set (server uses this pattern then unsets transparency client-side)
    Hidden,
    /// Creature/alternate full-body sprite override (head/boots colors only)
    Sprite {
        sprite: u16, // already offset-adjusted
        head_color: u8,
        boots_color: u8,
    },
    /// Ghost (dead) representation
    Dead {
        head_sprite: u16,
        body_sprite: u8, // base ghost body sprite (48 male ghost / 64 female ghost)
        is_transparent: bool,
        face_sprite: u8,
        is_male: bool,
    },
    /// Normal equipment-based appearance
    Normal {
        head_sprite: u16,
        body_sprite: u8, // base body sprite (multiple of 16) after removing pants color
        pants_color: u8, // 0 if none
        armor_sprite1: u16,
        boots_sprite: u8,
        armor_sprite2: u16,
        shield_sprite: u8,
        weapon_sprite: u16,
        head_color: u8,
        boots_color: u8,
        accessory_color1: u8,
        accessory_sprite1: u16,
        accessory_color2: u8,
        accessory_sprite2: u16,
        accessory_color3: u8,
        accessory_sprite3: u16,
        lantern_size: u8,
        rest_position: u8,
        overcoat_sprite: u16,
        overcoat_color: u8,
        body_color: u8,
        is_transparent: bool,
        face_sprite: u8,
        is_male: bool,
    },
}

impl Default for DisplayArgs {
    fn default() -> Self {
        DisplayArgs::Normal {
            head_sprite: 0,
            body_sprite: 1,
            pants_color: 0,
            armor_sprite1: 0,
            boots_sprite: 0,
            armor_sprite2: 0,
            shield_sprite: 0,
            weapon_sprite: 0,
            head_color: 0,
            boots_color: 0,
            accessory_color1: 0,
            accessory_sprite1: 0,
            accessory_color2: 0,
            accessory_sprite2: 0,
            accessory_color3: 0,
            accessory_sprite3: 0,
            lantern_size: 0,
            rest_position: 0,
            overcoat_sprite: 0,
            overcoat_color: 0,
            body_color: 0,
            is_transparent: false,
            face_sprite: 0,
            is_male: true,
        }
    }
}

impl DisplayArgs {
    pub fn body_sprite(&self) -> u8 {
        match self {
            DisplayArgs::Normal { body_sprite, .. } => *body_sprite,
            DisplayArgs::Dead { body_sprite, .. } => *body_sprite,
            _ => 0,
        }
    }

    pub fn is_male(&self) -> bool {
        match self {
            DisplayArgs::Normal { is_male, .. } => *is_male,
            DisplayArgs::Dead { is_male, .. } => *is_male,
            _ => true,
        }
    }

    pub fn normal() -> Self {
        Self::default()
    }

    pub fn with_armor(mut self, armor: u16) -> Self {
        if let DisplayArgs::Normal { armor_sprite1, .. } = &mut self {
            *armor_sprite1 = armor;
        }
        self
    }

    pub fn with_armor2(mut self, armor: u16) -> Self {
        if let DisplayArgs::Normal { armor_sprite2, .. } = &mut self {
            *armor_sprite2 = armor;
        }
        self
    }

    pub fn with_weapon(mut self, weapon: u16) -> Self {
        if let DisplayArgs::Normal { weapon_sprite, .. } = &mut self {
            *weapon_sprite = weapon;
        }
        self
    }

    pub fn with_shield(mut self, shield: u8) -> Self {
        if let DisplayArgs::Normal { shield_sprite, .. } = &mut self {
            *shield_sprite = shield;
        }
        self
    }

    pub fn with_helmet(mut self, helmet: u16, color: u8) -> Self {
        if let DisplayArgs::Normal {
            head_sprite,
            head_color,
            ..
        } = &mut self
        {
            *head_sprite = helmet;
            *head_color = color;
        }
        self
    }

    pub fn with_body(mut self, body: u8, color: u8) -> Self {
        if let DisplayArgs::Normal {
            body_sprite,
            body_color,
            ..
        } = &mut self
        {
            *body_sprite = body;
            *body_color = color;
        }
        self
    }

    pub fn with_boots(mut self, boots: u8, color: u8) -> Self {
        if let DisplayArgs::Normal {
            boots_sprite,
            boots_color,
            ..
        } = &mut self
        {
            *boots_sprite = boots;
            *boots_color = color;
        }
        self
    }

    pub fn with_pants_color(mut self, color: u8) -> Self {
        if let DisplayArgs::Normal { pants_color, .. } = &mut self {
            *pants_color = color;
        }
        self
    }

    pub fn with_accessory1(mut self, sprite: u16, color: u8) -> Self {
        if let DisplayArgs::Normal {
            accessory_sprite1,
            accessory_color1,
            ..
        } = &mut self
        {
            *accessory_sprite1 = sprite;
            *accessory_color1 = color;
        }
        self
    }

    pub fn with_accessory2(mut self, sprite: u16, color: u8) -> Self {
        if let DisplayArgs::Normal {
            accessory_sprite2,
            accessory_color2,
            ..
        } = &mut self
        {
            *accessory_sprite2 = sprite;
            *accessory_color2 = color;
        }
        self
    }

    pub fn with_accessory3(mut self, sprite: u16, color: u8) -> Self {
        if let DisplayArgs::Normal {
            accessory_sprite3,
            accessory_color3,
            ..
        } = &mut self
        {
            *accessory_sprite3 = sprite;
            *accessory_color3 = color;
        }
        self
    }

    pub fn with_overcoat(mut self, sprite: u16, color: u8) -> Self {
        if let DisplayArgs::Normal {
            overcoat_sprite,
            overcoat_color,
            ..
        } = &mut self
        {
            *overcoat_sprite = sprite;
            *overcoat_color = color;
        }
        self
    }

    pub fn with_lantern(mut self, size: u8) -> Self {
        if let DisplayArgs::Normal { lantern_size, .. } = &mut self {
            *lantern_size = size;
        }
        self
    }

    pub fn with_rest_position(mut self, position: u8) -> Self {
        if let DisplayArgs::Normal { rest_position, .. } = &mut self {
            *rest_position = position;
        }
        self
    }

    pub fn with_face(mut self, face: u8) -> Self {
        if let DisplayArgs::Normal { face_sprite, .. } = &mut self {
            *face_sprite = face;
        }
        self
    }

    pub fn with_transparent(mut self, transparent: bool) -> Self {
        if let DisplayArgs::Normal { is_transparent, .. } = &mut self {
            *is_transparent = transparent;
        }
        self
    }

    pub fn with_gender(mut self, gender: bool) -> Self {
        if let DisplayArgs::Normal { is_male, .. } = &mut self {
            *is_male = gender;
        }
        self
    }

    pub fn male(self) -> Self {
        self.with_gender(true)
    }

    pub fn female(self) -> Self {
        self.with_gender(false)
    }
}

#[derive(Debug, Clone)]
pub struct DisplayPlayer {
    pub x: u16,
    pub y: u16,
    pub direction: u8,
    pub id: u32,
    pub name_tag_style: u8,
    pub name: String,
    pub group_box_text: String,
    pub args: DisplayArgs,
}

impl Default for DisplayPlayer {
    fn default() -> Self {
        DisplayPlayer {
            x: 0,
            y: 0,
            direction: 0,
            id: 0,
            name_tag_style: 0,
            name: String::new(),
            group_box_text: String::new(),
            args: DisplayArgs::default(),
        }
    }
}

impl TryFromBytes for DisplayPlayer {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let point = (
            cursor.read_u16::<BigEndian>()?,
            cursor.read_u16::<BigEndian>()?,
        );
        let direction = cursor.read_u8()?;
        let id = cursor.read_u32::<BigEndian>()?;

        let head_sprite_read = cursor.read_u16::<BigEndian>()?;
        let args = if head_sprite_read == u16::MAX {
            let sprite = cursor.read_u16::<BigEndian>()? & !CREATURE_SPRITE_OFFSET;
            let head_color = cursor.read_u8()?;
            let boots_color = cursor.read_u8()?;
            // skip 6 unknown bytes
            let mut skip = [0u8; 6];
            cursor.read_exact(&mut skip)?;
            DisplayArgs::Sprite {
                sprite,
                head_color,
                boots_color,
            }
        } else {
            let mut body_sprite = cursor.read_u8()?;
            let armor_sprite1 = cursor.read_u16::<BigEndian>()?;
            let boots_sprite = cursor.read_u8()?;
            let armor_sprite2 = cursor.read_u16::<BigEndian>()?;
            let shield_sprite = cursor.read_u8()?;
            let weapon_sprite = cursor.read_u16::<BigEndian>()?;
            let head_color = cursor.read_u8()?;
            let boots_color = cursor.read_u8()?;
            let accessory_color1 = cursor.read_u8()?;
            let accessory_sprite1 = cursor.read_u16::<BigEndian>()?;
            let accessory_color2 = cursor.read_u8()?;
            let accessory_sprite2 = cursor.read_u16::<BigEndian>()?;
            let accessory_color3 = cursor.read_u8()?;
            let accessory_sprite3 = cursor.read_u16::<BigEndian>()?;
            let lantern_size = cursor.read_u8()?;
            let rest_position = cursor.read_u8()?;
            let overcoat_sprite = cursor.read_u16::<BigEndian>()?;
            let overcoat_color = cursor.read_u8()?;
            let body_color = cursor.read_u8()?;
            let is_transparent_raw = cursor.read_u8()? != 0;
            let face_sprite = cursor.read_u8()?;

            // Extract pants color from low 4 bits (mod 16) then reduce to base body sprite.
            let pants_color = body_sprite % 16; // 0..15
            if pants_color != 0 {
                body_sprite -= pants_color;
            }

            // Gender derivation from base body sprite groups (multiples of 16)
            let is_male = matches!(body_sprite, 16 | 48 | 80 | 112 | 128 | 160);
            let is_dead = matches!(body_sprite, 48 | 64); // ghost sprites

            // Hidden pattern: body none + transparency bit set -> hidden (transparency flag consumed)
            if body_sprite == 0 && is_transparent_raw {
                DisplayArgs::Hidden
            } else if is_dead {
                DisplayArgs::Dead {
                    head_sprite: head_sprite_read,
                    body_sprite,
                    is_transparent: is_transparent_raw,
                    face_sprite,
                    is_male,
                }
            } else {
                DisplayArgs::Normal {
                    head_sprite: head_sprite_read,
                    body_sprite, // base value (multiple of 16)
                    pants_color, // color component (0 if none)
                    armor_sprite1,
                    boots_sprite,
                    armor_sprite2,
                    shield_sprite,
                    weapon_sprite,
                    head_color,
                    boots_color,
                    accessory_color1,
                    accessory_sprite1,
                    accessory_color2,
                    accessory_sprite2,
                    accessory_color3,
                    accessory_sprite3,
                    lantern_size,
                    rest_position,
                    overcoat_sprite,
                    overcoat_color,
                    body_color,
                    is_transparent: is_transparent_raw,
                    face_sprite,
                    is_male,
                }
            }
        };

        let name_tag_style = cursor.read_u8()?;
        let name = {
            let mut buf = vec![0; cursor.read_u8()? as usize];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode name: {}", e))?
        };
        let group_box_text = {
            let mut buf = vec![0; cursor.read_u8()? as usize];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode group_box_text: {}", e))?
        };

        Ok(DisplayPlayer {
            x: point.0,
            y: point.1,
            direction,
            id,
            name_tag_style,
            name,
            group_box_text,
            args,
        })
    }
}
