use etagere::Allocation;
use formats::epf::{AnimationDirection, EpfAnimation, EpfAnimationType};
use glam::Vec2;
use rustc_hash::FxHashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Gender {
    Male,
    Female,
    Unisex,
}

impl Gender {
    pub fn char(&self) -> char {
        match self {
            Self::Female => 'w',
            _ => 'm',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PlayerPieceType {
    Accessory1Bg,
    Accessory1Fg,
    Accessory2Bg,
    Accessory2Fg,
    Accessory3Bg,
    Accessory3Fg,
    HelmetExtra,
    HelmetBg,
    HelmetFg,
    Boots,
    Body,
    Pants,
    Face,
    Shield,
    Armor,
    Arms,
    Weapon,
    Emote,
}

impl PlayerPieceType {
    pub const fn prefix(&self, id: u16) -> char {
        match self {
            PlayerPieceType::HelmetExtra => 'f',
            PlayerPieceType::HelmetBg => 'h',
            PlayerPieceType::HelmetFg => 'e',
            PlayerPieceType::Body => 'm',
            PlayerPieceType::Arms => {
                if id > 999 {
                    'j'
                } else {
                    'a'
                }
            }
            PlayerPieceType::Shield => 's',
            PlayerPieceType::Pants => 'n',
            PlayerPieceType::Armor => {
                if id > 999 {
                    'i'
                } else {
                    'u'
                }
            }
            PlayerPieceType::Boots => 'l',
            PlayerPieceType::Weapon => 'w',
            PlayerPieceType::Accessory1Bg
            | PlayerPieceType::Accessory2Bg
            | PlayerPieceType::Accessory3Bg => 'g',
            PlayerPieceType::Accessory1Fg
            | PlayerPieceType::Accessory2Fg
            | PlayerPieceType::Accessory3Fg => 'c',
            PlayerPieceType::Face => 'o',
            PlayerPieceType::Emote => ' ', // Not used for path construction
        }
    }

    pub const fn z_priority(&self, towards: bool) -> f32 {
        match towards {
            true => match self {
                PlayerPieceType::Accessory1Fg => 0.93,
                PlayerPieceType::Accessory2Fg => 0.92,
                PlayerPieceType::Accessory3Fg => 0.91,
                PlayerPieceType::Shield => 0.7,
                PlayerPieceType::Arms => 0.6,
                PlayerPieceType::Weapon => 0.5,
                PlayerPieceType::HelmetFg => 0.42,
                PlayerPieceType::Armor => 0.38,
                PlayerPieceType::HelmetBg => 0.3,
                PlayerPieceType::Boots => 0.26,
                PlayerPieceType::Emote => 0.23, // Slightly above face
                PlayerPieceType::Face => 0.22,
                PlayerPieceType::Pants => 0.18,
                PlayerPieceType::Body => 0.12,
                PlayerPieceType::Accessory1Bg => 0.09,
                PlayerPieceType::Accessory2Bg => 0.08,
                PlayerPieceType::Accessory3Bg => 0.07,
                PlayerPieceType::HelmetExtra => 0.06,
            },
            false => match self {
                PlayerPieceType::Accessory1Fg => 0.93,
                PlayerPieceType::Accessory2Fg => 0.92,
                PlayerPieceType::Accessory3Fg => 0.91,
                PlayerPieceType::Arms => 0.7,
                PlayerPieceType::Weapon => 0.6,
                PlayerPieceType::HelmetExtra => 0.55,
                PlayerPieceType::HelmetBg => 0.5,
                PlayerPieceType::HelmetFg => 0.45,
                PlayerPieceType::Armor => 0.4,
                PlayerPieceType::Boots => 0.325,
                PlayerPieceType::Pants => 0.28,
                PlayerPieceType::Emote => 0.245, // Slightly above face
                PlayerPieceType::Face => 0.24,
                PlayerPieceType::Body => 0.2,
                PlayerPieceType::Shield => 0.15,
                PlayerPieceType::Accessory1Bg => 0.09,
                PlayerPieceType::Accessory2Bg => 0.08,
                PlayerPieceType::Accessory3Bg => 0.07,
            },
        }
    }

    pub const fn offset(&self) -> Vec2 {
        match self {
            PlayerPieceType::Weapon => Vec2::new(-27., 0.),
            PlayerPieceType::Accessory1Fg
            | PlayerPieceType::Accessory2Fg
            | PlayerPieceType::Accessory3Fg
            | PlayerPieceType::Accessory1Bg
            | PlayerPieceType::Accessory2Bg
            | PlayerPieceType::Accessory3Bg => Vec2::new(-27., 0.),
            _ => Vec2::new(0., 0.),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerSpriteKey {
    pub gender: Gender,
    pub slot: PlayerPieceType,
    pub sprite_id: u16,
}

impl PlayerSpriteKey {
    pub fn prefix_for_palette(&self, sprite_id: u16) -> char {
        match self.slot {
            PlayerPieceType::Shield => 'w',
            PlayerPieceType::Pants => 'b',
            PlayerPieceType::Accessory1Bg
            | PlayerPieceType::Accessory2Bg
            | PlayerPieceType::Accessory3Bg => 'c',
            PlayerPieceType::Face | PlayerPieceType::Emote => 'm',
            PlayerPieceType::Arms => 'u',
            PlayerPieceType::Armor => 'i',
            _ => self.slot.prefix(sprite_id),
        }
    }

    pub fn for_piece(slot: PlayerPieceType, sprite_id: u16, gender: Gender) -> Self {
        Self {
            slot,
            sprite_id,
            gender: match slot {
                PlayerPieceType::Shield | PlayerPieceType::Emote => Gender::Male,
                _ => gender,
            },
        }
    }
}

pub(crate) struct LoadedSprite {
    pub epf_image: Vec<EpfAnimation>,
    pub allocations: Vec<Option<Allocation>>,
    pub animations: FxHashMap<(EpfAnimationType, AnimationDirection), AnimationData>,
    pub ref_count: usize,
}

pub struct AnimationData {
    pub frame_count: usize,
    pub start_frame_index: usize,
    pub epf_index: usize,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PlayerSpriteIndex(pub(crate) usize);
impl PlayerSpriteIndex {
    pub fn index(&self) -> usize {
        self.0
    }
}
#[derive(Copy, Clone, Debug)]
pub struct PlayerSpriteHandle {
    pub key: PlayerSpriteKey,
    pub index: PlayerSpriteIndex,
    pub stack_order: u8,
}
