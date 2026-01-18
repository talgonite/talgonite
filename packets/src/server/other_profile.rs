use crate::TryFromBytes;
use crate::server::EquipmentSlot;
use crate::types::{ItemInfo, LegendMarkInfo, MarkColor, MarkIcon, Nation, SocialStatus};
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use std::io::{Cursor, Read};

#[derive(Debug, Clone)]
pub struct OtherProfile {
    pub id: u32,
    pub equipment: std::collections::HashMap<EquipmentSlot, ItemInfo>,
    pub social_status: SocialStatus,
    pub name: String,
    pub nation: Nation,
    pub title: String,
    pub group_open: bool,
    pub guild_rank: String,
    pub display_class: String,
    pub guild_name: String,
    pub legend_marks: Vec<LegendMarkInfo>,
    pub portrait: Option<Vec<u8>>,
    pub profile_text: Option<String>,
}

impl TryFromBytes for OtherProfile {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let id = cursor.read_u32::<BigEndian>()?;
        let mut equipment = std::collections::HashMap::new();

        // The order matters here, it must match PROFILE_EQUIPMENTSLOT_ORDER from C#
        for slot in EquipmentSlot::iter() {
            let sprite = cursor.read_u16::<BigEndian>()?;
            let color = cursor.read_u8()?;
            if sprite != 0 {
                // Apply the same offset logic as standard equipment packets
                let sprite = if sprite >= crate::server::ITEM_SPRITE_OFFSET {
                    sprite - crate::server::ITEM_SPRITE_OFFSET
                } else {
                    sprite
                };
                equipment.insert(slot, ItemInfo { sprite, color });
            }
        }

        let social_status =
            SocialStatus::try_from(cursor.read_u8()?).unwrap_or(SocialStatus::Awake);

        let read_string = |cursor: &mut Cursor<&[u8]>| -> anyhow::Result<String> {
            let len = cursor.read_u8()?;
            let mut buf = vec![0; len as usize];
            cursor.read_exact(&mut buf)?;
            Ok(WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .unwrap_or_default())
        };

        let name = read_string(&mut cursor)?;

        let nation = Nation::try_from(cursor.read_u8()?).unwrap_or(Nation::Exile);

        let title = read_string(&mut cursor)?;
        let group_open = cursor.read_u8()? != 0;
        let guild_rank = read_string(&mut cursor)?;
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

        let remaining = cursor.read_u16::<BigEndian>()?;
        let mut portrait = None;
        let mut profile_text = None;

        if remaining > 0 {
            let portrait_len = cursor.read_u16::<BigEndian>()?;
            let mut portrait_buf = vec![0; portrait_len as usize];
            cursor.read_exact(&mut portrait_buf)?;
            portrait = Some(portrait_buf);

            let profile_text_len = cursor.read_u16::<BigEndian>()?;
            let mut profile_text_buf = vec![0; profile_text_len as usize];
            cursor.read_exact(&mut profile_text_buf)?;
            profile_text = Some(
                WINDOWS_949
                    .decode(&profile_text_buf, DecoderTrap::Replace)
                    .unwrap_or_default(),
            );
        }

        Ok(OtherProfile {
            id,
            equipment,
            social_status,
            name,
            nation,
            title,
            group_open,
            guild_rank,
            display_class,
            guild_name,
            legend_marks,
            portrait,
            profile_text,
        })
    }
}
