use crate::TryFromBytes;
use byteorder::{BE, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use num_enum::{Default, IntoPrimitive, TryFromPrimitive};
use std::io::{Cursor, Read};

const ITEM_SPRITE_OFFSET: u16 = 0x8000;
const CREATURE_SPRITE_OFFSET: u16 = 0x4000;

#[derive(Debug, Clone)]
pub enum EntityInfo {
    Item {
        x: u16,
        y: u16,
        id: u32,
        sprite: u16,
        color: u8,
    },
    Creature {
        x: u16,
        y: u16,
        id: u32,
        sprite: u16,
        direction: u8,
        entity_type: VisibleEntityType,
        name: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub struct DisplayVisibleEntities {
    pub entities: Vec<EntityInfo>,
}

impl std::default::Default for DisplayVisibleEntities {
    fn default() -> Self {
        DisplayVisibleEntities {
            entities: Vec::new(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum VisibleEntityType {
    Normal = 0,
    WalkThrough = 1,
    Merchant = 2,
    WhiteSquare = 3,
    Aisling = 4,
    #[default]
    Unknown = 255,
}

impl TryFromBytes for DisplayVisibleEntities {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let count = cursor.read_u16::<BE>()?;
        let mut entities = Vec::with_capacity(count as usize);

        for _ in 0..count {
            let x = cursor.read_u16::<BE>()?;
            let y = cursor.read_u16::<BE>()?;
            let id = cursor.read_u32::<BE>()?;
            let sprite = cursor.read_u16::<BE>()?;

            let entity_info = match sprite {
                sprite if sprite >= ITEM_SPRITE_OFFSET => {
                    let color = cursor.read_u8()?;
                    let mut _discard = [0u8; 2];
                    cursor.read_exact(&mut _discard)?;
                    EntityInfo::Item {
                        x,
                        y,
                        id,
                        sprite: sprite - ITEM_SPRITE_OFFSET,
                        color,
                    }
                }
                sprite if sprite >= CREATURE_SPRITE_OFFSET => {
                    let mut _discard = [0u8; 4];
                    cursor.read_exact(&mut _discard)?; // ??
                    let direction = cursor.read_u8()?;
                    let _ = cursor.read_u8()?; // ??
                    let entity_type: VisibleEntityType = cursor.read_u8()?.try_into()?;
                    let name =
                        match entity_type {
                            VisibleEntityType::Merchant => {
                                let mut buf = vec![0; cursor.read_u8()? as usize];
                                cursor.read_exact(&mut buf)?;
                                Some(WINDOWS_949.decode(&buf, DecoderTrap::Replace).map_err(
                                    |e| anyhow::anyhow!("Failed to decode name: {:?}", e),
                                )?)
                            }
                            _ => None,
                        };
                    EntityInfo::Creature {
                        x,
                        y,
                        id,
                        sprite: sprite - CREATURE_SPRITE_OFFSET,
                        direction,
                        entity_type,
                        name,
                    }
                }
                _ => anyhow::bail!("Unknown sprite type: {}", sprite),
            };
            entities.push(entity_info);
        }

        Ok(DisplayVisibleEntities { entities })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_display_visible_entities() {
        let bytes: Vec<u8> = vec![
            0, 2, 0, 3, 0, 3, 0, 0, 30, 93, 66, 4, 0, 0, 0, 0, 0, 0, 2, 5, 77, 97, 114, 105, 97, 0,
            3, 0, 8, 0, 13, 193, 194, 128, 45, 0, 0, 0,
        ];

        let entities = DisplayVisibleEntities::try_from_bytes(&bytes).unwrap();
        assert_eq!(entities.entities.len(), 2);

        if let EntityInfo::Creature {
            x,
            y,
            id,
            sprite,
            direction,
            entity_type,
            name,
        } = &entities.entities[0]
        {
            assert_eq!(*x, 3);
            assert_eq!(*y, 3);
            assert_eq!(*id, 7773);
            assert_eq!(*sprite, 516);
            assert_eq!(*direction, 0);
            assert_eq!(*entity_type, VisibleEntityType::Merchant);
            assert_eq!(name.as_deref(), Some("Maria"));
        } else {
            panic!("Expected first entity to be a Creature");
        }

        if let EntityInfo::Item {
            x,
            y,
            id,
            sprite,
            color,
        } = &entities.entities[1]
        {
            assert_eq!(*x, 3);
            assert_eq!(*y, 8);
            assert_eq!(*id, 901570);
            assert_eq!(*sprite, 45);
            assert_eq!(*color, 0);
        } else {
            panic!("Expected second entity to be an Item");
        }
    }
}
