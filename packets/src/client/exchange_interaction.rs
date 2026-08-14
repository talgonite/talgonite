use crate::{ToBytes, TryFromBytes};
use anyhow::bail;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

use super::Codes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExchangeInteraction {
    Start {
        other_player_id: u32,
    },
    AddItem {
        other_player_id: u32,
        source_slot: u8,
    },
    AddStackableItem {
        other_player_id: u32,
        source_slot: u8,
        item_count: u8,
    },
    SetGold {
        other_player_id: u32,
        gold_amount: u32,
    },
    Cancel {
        other_player_id: u32,
    },
    Accept {
        other_player_id: u32,
    },
}

impl ExchangeInteraction {
    pub fn other_player_id(&self) -> u32 {
        match self {
            Self::Start { other_player_id }
            | Self::AddItem {
                other_player_id, ..
            }
            | Self::AddStackableItem {
                other_player_id, ..
            }
            | Self::SetGold {
                other_player_id, ..
            }
            | Self::Cancel { other_player_id }
            | Self::Accept { other_player_id } => *other_player_id,
        }
    }

    pub fn stage(&self) -> u8 {
        match self {
            Self::Start { .. } => 0,
            Self::AddItem { .. } => 1,
            Self::AddStackableItem { .. } => 2,
            Self::SetGold { .. } => 3,
            Self::Cancel { .. } => 4,
            Self::Accept { .. } => 5,
        }
    }
}

impl ToBytes for ExchangeInteraction {
    const OPCODE: u8 = Codes::ExchangeInteraction as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.stage());
        bytes.extend_from_slice(&self.other_player_id().to_be_bytes());

        match self {
            Self::AddItem { source_slot, .. } => {
                bytes.push(*source_slot);
            }
            Self::AddStackableItem {
                source_slot,
                item_count,
                ..
            } => {
                bytes.push(*source_slot);
                bytes.push(*item_count);
            }
            Self::SetGold { gold_amount, .. } => {
                bytes.extend_from_slice(&gold_amount.to_be_bytes());
            }
            Self::Start { .. } | Self::Cancel { .. } | Self::Accept { .. } => {}
        }
    }
}

impl TryFromBytes for ExchangeInteraction {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let stage = cursor.read_u8()?;
        let other_player_id = cursor.read_u32::<BigEndian>()?;

        match stage {
            0 => Ok(Self::Start { other_player_id }),
            1 => Ok(Self::AddItem {
                other_player_id,
                source_slot: cursor.read_u8()?,
            }),
            2 => Ok(Self::AddStackableItem {
                other_player_id,
                source_slot: cursor.read_u8()?,
                item_count: cursor.read_u8()?,
            }),
            3 => Ok(Self::SetGold {
                other_player_id,
                gold_amount: cursor.read_u32::<BigEndian>()?,
            }),
            4 => Ok(Self::Cancel { other_player_id }),
            5 => Ok(Self::Accept { other_player_id }),
            other => bail!("Invalid exchange request stage: {other}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exchange_interaction_round_trip() {
        let packets = vec![
            ExchangeInteraction::Start {
                other_player_id: 12345,
            },
            ExchangeInteraction::AddItem {
                other_player_id: 12345,
                source_slot: 3,
            },
            ExchangeInteraction::AddStackableItem {
                other_player_id: 12345,
                source_slot: 5,
                item_count: 10,
            },
            ExchangeInteraction::SetGold {
                other_player_id: 12345,
                gold_amount: 50000,
            },
            ExchangeInteraction::Cancel {
                other_player_id: 12345,
            },
            ExchangeInteraction::Accept {
                other_player_id: 12345,
            },
        ];

        for pkt in packets {
            let mut bytes = Vec::new();
            pkt.write_payload(&mut bytes);
            let parsed = ExchangeInteraction::try_from_bytes(&bytes).unwrap();
            assert_eq!(pkt, parsed);
        }
    }
}
