use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::ReadBytesExt;
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::io::Read;

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum WorldListColor {
    Guilded = 84,
    NotSure = 88,
    Unknown = 144,
    WithinLevelRange = 151,
    White = 255,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum SocialStatus {
    Awake = 0,
    DoNotDisturb = 1,
    DayDreaming = 2,
    NeedGroup = 3,
    Grouped = 4,
    LoneHunter = 5,
    GroupHunting = 6,
    NeedHelp = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum BaseClass {
    Peasant = 0,
    Warrior = 1,
    Rogue = 2,
    Wizard = 3,
    Priest = 4,
    Monk = 5,
}

#[derive(Debug, Clone)]
pub struct WorldListMember {
    pub base_class: BaseClass,
    pub color: WorldListColor,
    pub social_status: SocialStatus,
    pub title: String,
    pub is_master: bool,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct WorldList {
    pub world_member_count: u16,
    pub country_list: Vec<WorldListMember>,
}

impl TryFromBytes for WorldList {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = std::io::Cursor::new(bytes);
        let world_member_count = cursor.read_u16::<byteorder::BigEndian>()?;
        let country_count = cursor.read_u16::<byteorder::BigEndian>()?;
        let mut country_list = Vec::with_capacity(country_count as usize);
        for _ in 0..country_count {
            let base_class_raw = cursor.read_u8()?;
            let base_class = (base_class_raw & 0b111).try_into()?;
            let color = cursor.read_u8()?.try_into()?;
            let social_status = cursor.read_u8()?.try_into()?;
            let title = {
                let mut buf = vec![0; cursor.read_u8()? as usize];
                cursor.read_exact(&mut buf)?;
                WINDOWS_949
                    .decode(&buf, DecoderTrap::Replace)
                    .map_err(|e| anyhow!("Failed to decode title: {}", e))?
            };
            let is_master = cursor.read_u8()? != 0;
            let name = {
                let mut buf = vec![0; cursor.read_u8()? as usize];
                cursor.read_exact(&mut buf)?;
                WINDOWS_949
                    .decode(&buf, DecoderTrap::Replace)
                    .map_err(|e| anyhow!("Failed to decode name: {}", e))?
            };

            country_list.push(WorldListMember {
                base_class,
                color,
                social_status,
                title,
                is_master,
                name,
            });
        }
        Ok(WorldList {
            world_member_count,
            country_list,
        })
    }
}
