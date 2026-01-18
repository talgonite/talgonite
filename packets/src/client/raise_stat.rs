use num_enum::IntoPrimitive;

use crate::ToBytes;

use super::Codes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive)]
#[repr(u8)]
pub enum Stat {
    STR = 1,
    DEX = 2,
    INT = 4,
    WIS = 8,
    CON = 16,
}

#[derive(Debug)]
pub struct RaiseStat {
    pub stat: Stat,
}

impl ToBytes for RaiseStat {
    const OPCODE: u8 = Codes::RaiseStat as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.stat.into());
    }
}
