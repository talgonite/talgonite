use crate::TryFromBytes;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor, Read};

#[derive(Debug)]
pub struct AttributesPrimary {
    pub level: u8,
    pub ability: u8,
    pub maximum_hp: u32,
    pub maximum_mp: u32,
    pub str: u8,
    pub int: u8,
    pub wis: u8,
    pub con: u8,
    pub dex: u8,
    pub unspent_points: u8,
    pub max_weight: i16,
    pub current_weight: i16,
}

#[derive(Debug)]
pub struct AttributesVitality {
    pub current_hp: u32,
    pub current_mp: u32,
}

#[derive(Debug)]
pub struct AttributesExpGold {
    pub total_exp: u32,
    pub to_next_level: u32,
    pub total_ability: u32,
    pub to_next_ability: u32,
    pub game_points: u32,
    pub gold: u32,
}

#[derive(Debug)]
pub struct AttributesSecondary {
    pub blind: bool,
    pub has_unread_mail: bool,
    pub offense_element: u8,
    pub defense_element: u8,
    pub magic_resistance: u8,
    pub ac: i8,
    pub dmg: u8,
    pub hit: u8,
}

#[derive(Debug)]
pub struct Attributes {
    pub is_admin_a: bool,
    pub is_admin_b: bool,
    pub is_swimming: bool,
    pub has_unread_mail_flag: bool, // from top-level flag bit 0
    pub primary: Option<AttributesPrimary>,
    pub vitality: Option<AttributesVitality>,
    pub exp_gold: Option<AttributesExpGold>,
    pub secondary: Option<AttributesSecondary>,
}

impl TryFromBytes for Attributes {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let flags = cursor.read_u8()?;

        // Flag bits based on C# StatUpdateType
        const UNREAD_MAIL: u8 = 1 << 0; // also reused inside secondary block as separate byte later
        const SECONDARY: u8 = 1 << 2;
        const EXP_GOLD: u8 = 1 << 3;
        const VITALITY: u8 = 1 << 4;
        const PRIMARY: u8 = 1 << 5;
        const GM_A: u8 = 1 << 6;
        const GM_B: u8 = 1 << 7;

        let is_admin_a = flags & GM_A != 0;
        let is_admin_b = flags & GM_B != 0;
        let is_swimming = (flags & (GM_A | GM_B)) == (GM_A | GM_B); // Swimming alias
        let has_unread_mail_flag = flags & UNREAD_MAIL != 0; // top-level flag only

        // Parse sections in same order as server serialization: Primary, Vitality, ExpGold, Secondary
        let primary = if flags & PRIMARY != 0 {
            // Skip 3 unknown/padding bytes
            let mut skip = [0u8; 3];
            cursor.read_exact(&mut skip)?;
            let level = cursor.read_u8()?;
            let ability = cursor.read_u8()?;
            let maximum_hp = cursor.read_u32::<BigEndian>()?;
            let maximum_mp = cursor.read_u32::<BigEndian>()?;
            let str = cursor.read_u8()?;
            let int = cursor.read_u8()?;
            let wis = cursor.read_u8()?;
            let con = cursor.read_u8()?;
            let dex = cursor.read_u8()?;
            // has_unspent_points bool (discard, derive from unspent_points)
            let _has_unspent_bool = cursor.read_u8()?;
            let unspent_points = cursor.read_u8()?;
            let max_weight = cursor.read_i16::<BigEndian>()?;
            let current_weight = cursor.read_i16::<BigEndian>()?;
            // trailing 4 unknown bytes
            let mut trailing = [0u8; 4];
            cursor.read_exact(&mut trailing)?;
            Some(AttributesPrimary {
                level,
                ability,
                maximum_hp,
                maximum_mp,
                str,
                int,
                wis,
                con,
                dex,
                unspent_points,
                max_weight,
                current_weight,
            })
        } else {
            None
        };

        let vitality = if flags & VITALITY != 0 {
            Some(AttributesVitality {
                current_hp: cursor.read_u32::<BigEndian>()?,
                current_mp: cursor.read_u32::<BigEndian>()?,
            })
        } else {
            None
        };

        let exp_gold = if flags & EXP_GOLD != 0 {
            Some(AttributesExpGold {
                total_exp: cursor.read_u32::<BigEndian>()?,
                to_next_level: cursor.read_u32::<BigEndian>()?,
                total_ability: cursor.read_u32::<BigEndian>()?,
                to_next_ability: cursor.read_u32::<BigEndian>()?,
                game_points: cursor.read_u32::<BigEndian>()?,
                gold: cursor.read_u32::<BigEndian>()?,
            })
        } else {
            None
        };

        let secondary = if flags & SECONDARY != 0 {
            // Layout per C# serializer
            // 1 unknown byte
            let _unknown1 = cursor.read_u8()?;
            // blind stored as 8 if true else 0
            let blind = cursor.read_u8()? == 8;
            // 3 unknown bytes
            let mut unk3 = [0u8; 3];
            cursor.read_exact(&mut unk3)?;
            // mail flag byte (16 if has mail)
            let has_unread_mail = cursor.read_u8()? == 16;
            let offense_element = cursor.read_u8()?;
            let defense_element = cursor.read_u8()?;
            let magic_resistance = cursor.read_u8()?;
            // 1 unknown byte
            let _unknown2 = cursor.read_u8()?;
            let ac = cursor.read_i8()?;
            let dmg = cursor.read_u8()?;
            let hit = cursor.read_u8()?;
            Some(AttributesSecondary {
                blind,
                has_unread_mail,
                offense_element,
                defense_element,
                magic_resistance,
                ac,
                dmg,
                hit,
            })
        } else {
            None
        };

        Ok(Attributes {
            is_admin_a,
            is_admin_b,
            is_swimming,
            has_unread_mail_flag,
            primary,
            vitality,
            exp_gold,
            secondary,
        })
    }
}
