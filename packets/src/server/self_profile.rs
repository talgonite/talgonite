use crate::TryFromBytes;
use crate::types::{LegendMarkInfo, MarkColor, MarkIcon, Nation};
use byteorder::ReadBytesExt;
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use std::io::{Cursor, Read, Seek, SeekFrom};

#[derive(Debug, Clone)]
pub struct SelfProfile {
    pub nation: Nation,
    pub guild_rank: String,
    pub title: String,
    pub group_string: String,
    pub group_open: bool,
    pub group_box: bool,
    pub base_class: u8,
    pub enable_master_ability_metadata: bool,
    pub enable_master_quest_metadata: bool,
    pub display_class: String,
    pub guild_name: String,
    pub legend_marks: Vec<LegendMarkInfo>,
}

impl TryFromBytes for SelfProfile {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let nation = Nation::try_from(cursor.read_u8()?).unwrap_or(Nation::Exile);

        let read_string = |cursor: &mut Cursor<&[u8]>| -> anyhow::Result<String> {
            let len = cursor.read_u8()?;
            let mut buf = vec![0; len as usize];
            cursor.read_exact(&mut buf)?;
            Ok(WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .unwrap_or_default())
        };

        let guild_rank = read_string(&mut cursor)?;
        let title = read_string(&mut cursor)?;
        let group_string = read_string(&mut cursor)?;
        let group_open = cursor.read_u8()? != 0;
        let group_box = cursor.read_u8()? != 0;

        if group_box {
            // Skip leader name
            let len = cursor.read_u8()?;
            cursor.seek(SeekFrom::Current(len as i64))?;
            // Skip box text
            let len = cursor.read_u8()?;
            cursor.seek(SeekFrom::Current(len as i64))?;
            // Skip 13 bytes
            cursor.seek(SeekFrom::Current(13))?;
        }

        let base_class = cursor.read_u8()?;
        let enable_master_ability_metadata = cursor.read_u8()? != 0;
        let enable_master_quest_metadata = cursor.read_u8()? != 0;
        let display_class = read_string(&mut cursor)?;
        let guild_name = read_string(&mut cursor)?;
        let legend_mark_count = cursor.read_u8()?;
        let mut legend_marks = Vec::with_capacity(legend_mark_count as usize);
        for _ in 0..legend_mark_count {
            let icon = MarkIcon::try_from(cursor.read_u8()?).unwrap_or(MarkIcon::Yay);
            let color = MarkColor::try_from(cursor.read_u8()?).unwrap_or(MarkColor::Invisible);
            let key = read_string(&mut cursor)?;
            let text = read_string(&mut cursor)?;
            legend_marks.push(LegendMarkInfo {
                icon,
                color,
                key,
                text,
            });
        }
        Ok(SelfProfile {
            nation,
            guild_rank,
            title,
            group_string,
            group_open,
            group_box,
            base_class,
            enable_master_ability_metadata,
            enable_master_quest_metadata,
            display_class,
            guild_name,
            legend_marks,
        })
    }
}
