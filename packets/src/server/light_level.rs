use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::TryFromBytes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum LightLevelKind {
    DarkestA = 0,
    DarkerA = 1,
    DarkA = 2,
    LightA = 3,
    LighterA = 4,
    LightestA = 5,
    DarkestB = 6,
    DarkerB = 7,
    DarkB = 8,
    LightB = 9,
    LighterB = 10,
    LightestB = 11,
    Unknown = 255,
}

#[derive(Debug)]
pub struct LightLevel {
    pub kind: LightLevelKind,
}

impl TryFromBytes for LightLevel {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let byte = bytes.get(0).copied().unwrap_or(255);
        Ok(LightLevel {
            kind: byte.try_into()?,
        })
    }
}
