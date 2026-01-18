use crate::TryFromBytes;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

#[derive(Debug, Clone)]
pub enum Animation {
    Target {
        target_animation: u16,
        animation_speed: u16,
        target_point: (u16, u16),
    },
    Source {
        target_id: u32,
        source_id: u32,
        target_animation: Option<u16>,
        source_animation: Option<u16>,
        animation_speed: u16,
    },
}

impl TryFromBytes for Animation {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let target_id = cursor.read_u32::<BigEndian>()?;

        if target_id == 0 {
            let target_animation = cursor.read_u16::<BigEndian>()?;
            let animation_speed = cursor.read_u16::<BigEndian>()?;
            let target_point = {
                let x = cursor.read_u16::<BigEndian>()?;
                let y = cursor.read_u16::<BigEndian>()?;
                (x, y)
            };

            Ok(Animation::Target {
                target_animation,
                animation_speed,
                target_point,
            })
        } else {
            let source_id = cursor.read_u32::<BigEndian>()?;
            let target_animation = cursor.read_u16::<BigEndian>()?;
            let source_animation = cursor.read_u16::<BigEndian>()?;
            let animation_speed = cursor.read_u16::<BigEndian>()?;

            Ok(Animation::Source {
                target_id,
                source_id,
                target_animation: if target_animation == 0 {
                    None
                } else {
                    Some(target_animation)
                },
                source_animation: if source_animation == 0 {
                    None
                } else {
                    Some(source_animation)
                },
                animation_speed,
            })
        }
    }
}
