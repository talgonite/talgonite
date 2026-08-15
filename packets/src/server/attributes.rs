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
    pub has_unread_mail_flag: bool,
    pub primary: Option<AttributesPrimary>,
    pub vitality: Option<AttributesVitality>,
    pub exp_gold: Option<AttributesExpGold>,
    pub secondary: Option<AttributesSecondary>,
}

impl TryFromBytes for Attributes {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let flags = cursor.read_u8()?;

        const UNREAD_MAIL: u8 = 1 << 0;
        const SECONDARY: u8 = 1 << 2;
        const EXP_GOLD: u8 = 1 << 3;
        const VITALITY: u8 = 1 << 4;
        const PRIMARY: u8 = 1 << 5;
        const GM_A: u8 = 1 << 6;
        const GM_B: u8 = 1 << 7;

        let is_admin_a = flags & GM_A != 0;
        let is_admin_b = flags & GM_B != 0;
        let is_swimming = (flags & (GM_A | GM_B)) == (GM_A | GM_B);
        let has_unread_mail_flag = flags & UNREAD_MAIL != 0;

        let primary = if flags & PRIMARY != 0 {
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
            let _has_unspent_bool = cursor.read_u8()?;
            let unspent_points = cursor.read_u8()?;
            let max_weight = cursor.read_i16::<BigEndian>()?;
            let current_weight = cursor.read_i16::<BigEndian>()?;
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
            let _unknown1 = cursor.read_u8()?;
            let blind = cursor.read_u8()? == 8;
            let mut unk3 = [0u8; 3];
            cursor.read_exact(&mut unk3)?;
            let has_unread_mail = cursor.read_u8()? == 16;
            let offense_element = cursor.read_u8()?;
            let defense_element = cursor.read_u8()?;
            let magic_resistance = cursor.read_u8()?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attributes_primary_and_secondary() {
        let raw = vec![
            (1 << 5) | (1 << 2), // PRIMARY | SECONDARY flags
            // Primary block
            0,
            0,
            0,  // 3 skip bytes
            99, // level
            15, // ability
            0,
            0,
            0x10,
            0x00, // max_hp: 4096
            0,
            0,
            0x08,
            0x00, // max_mp: 2048
            25,   // str
            18,   // int
            20,   // wis
            30,   // con
            22,   // dex
            1,    // has_unspent_bool
            3,    // unspent_points
            0x01,
            0x00, // max_weight: 256
            0x00,
            0x50, // current_weight: 80
            0,
            0,
            0,
            0, // 4 trailing skip bytes
            // Secondary block
            0, // unknown1
            0, // blind (0 = false)
            0,
            0,
            0,           // unk3
            0,           // mail
            1,           // offense_element
            2,           // defense_element
            15,          // magic_resistance
            0,           // unknown2
            -45i8 as u8, // ac
            10,          // dmg
            12,          // hit
        ];

        let parsed = Attributes::try_from_bytes(&raw).expect("parse attributes");
        let primary = parsed.primary.expect("primary present");
        assert_eq!(primary.level, 99);
        assert_eq!(primary.ability, 15);
        assert_eq!(primary.maximum_hp, 4096);
        assert_eq!(primary.maximum_mp, 2048);
        assert_eq!(primary.str, 25);
        assert_eq!(primary.int, 18);
        assert_eq!(primary.wis, 20);
        assert_eq!(primary.con, 30);
        assert_eq!(primary.dex, 22);
        assert_eq!(primary.unspent_points, 3);
        assert_eq!(primary.max_weight, 256);
        assert_eq!(primary.current_weight, 80);

        let secondary = parsed.secondary.expect("secondary present");
        assert_eq!(secondary.ac, -45);
        assert_eq!(secondary.dmg, 10);
        assert_eq!(secondary.hit, 12);
        assert_eq!(secondary.magic_resistance, 15);
    }
}
