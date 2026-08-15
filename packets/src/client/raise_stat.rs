use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::ToBytes;

use super::Codes;

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum Stat {
    Str = 1,
    Dex = 2,
    Int = 4,
    Wis = 8,
    Con = 16,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raise_stat_serialization() {
        let pkt = RaiseStat { stat: Stat::Wis };
        let mut bytes = Vec::new();
        pkt.write_payload(&mut bytes);
        assert_eq!(bytes, vec![8]);

        assert_eq!(Stat::try_from(1), Ok(Stat::Str));
        assert_eq!(Stat::try_from(2), Ok(Stat::Dex));
        assert_eq!(Stat::try_from(4), Ok(Stat::Int));
        assert_eq!(Stat::try_from(8), Ok(Stat::Wis));
        assert_eq!(Stat::try_from(16), Ok(Stat::Con));
        assert!(Stat::try_from(99).is_err());
    }
}
