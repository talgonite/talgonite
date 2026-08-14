use crate::{ToBytes, TryFromBytes};
use anyhow::{anyhow, bail};
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, EncoderTrap, Encoding};
use std::io::{Cursor, Read};

use super::Codes;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayExchange {
    Start {
        other_user_id: u32,
        other_user_name: String,
    },
    RequestAmount {
        from_slot: u8,
    },
    AddItem {
        right_side: bool,
        exchange_index: u8,
        item_sprite: u16,
        item_color: u8,
        item_name: String,
    },
    SetGold {
        right_side: bool,
        gold_amount: u32,
    },
    Cancel {
        right_side: bool,
        message: String,
    },
    Accept {
        right_side: bool,
        message: String,
    },
}

impl DisplayExchange {
    pub fn action(&self) -> u8 {
        match self {
            Self::Start { .. } => 0,
            Self::RequestAmount { .. } => 1,
            Self::AddItem { .. } => 2,
            Self::SetGold { .. } => 3,
            Self::Cancel { .. } => 4,
            Self::Accept { .. } => 5,
        }
    }
}

impl ToBytes for DisplayExchange {
    const OPCODE: u8 = Codes::DisplayExchange as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        bytes.push(self.action());
        match self {
            Self::Start {
                other_user_id,
                other_user_name,
            } => {
                bytes.extend_from_slice(&other_user_id.to_be_bytes());
                let name_bytes = WINDOWS_949
                    .encode(other_user_name, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.push(name_bytes.len() as u8);
                bytes.extend_from_slice(&name_bytes);
            }
            Self::RequestAmount { from_slot } => {
                bytes.push(*from_slot);
            }
            Self::AddItem {
                right_side,
                exchange_index,
                item_sprite,
                item_color,
                item_name,
            } => {
                bytes.push(if *right_side { 1 } else { 0 });
                bytes.push(*exchange_index);
                let wire_sprite = if *item_sprite < 0x8000 {
                    *item_sprite | 0x8000
                } else {
                    *item_sprite
                };
                bytes.extend_from_slice(&wire_sprite.to_be_bytes());
                bytes.push(*item_color);
                let name_bytes = WINDOWS_949
                    .encode(item_name, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.push(name_bytes.len() as u8);
                bytes.extend_from_slice(&name_bytes);
            }
            Self::SetGold {
                right_side,
                gold_amount,
            } => {
                bytes.push(if *right_side { 1 } else { 0 });
                bytes.extend_from_slice(&gold_amount.to_be_bytes());
            }
            Self::Cancel {
                right_side,
                message,
            } => {
                bytes.push(if *right_side { 1 } else { 0 });
                let msg_bytes = WINDOWS_949
                    .encode(message, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.push(msg_bytes.len() as u8);
                bytes.extend_from_slice(&msg_bytes);
            }
            Self::Accept {
                right_side,
                message,
            } => {
                bytes.push(if *right_side { 1 } else { 0 });
                let msg_bytes = WINDOWS_949
                    .encode(message, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.push(msg_bytes.len() as u8);
                bytes.extend_from_slice(&msg_bytes);
            }
        }
    }
}

impl TryFromBytes for DisplayExchange {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let action = cursor.read_u8()?;

        let decode_string = |cursor: &mut Cursor<&[u8]>, label: &str| -> anyhow::Result<String> {
            let len = cursor.read_u8()? as usize;
            let mut buf = vec![0; len];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode {}: {}", label, e))
        };

        match action {
            0 => Ok(DisplayExchange::Start {
                other_user_id: cursor.read_u32::<BigEndian>()?,
                other_user_name: decode_string(&mut cursor, "other_user_name")?,
            }),
            1 => Ok(DisplayExchange::RequestAmount {
                from_slot: cursor.read_u8()?,
            }),
            2 => Ok(DisplayExchange::AddItem {
                right_side: cursor.read_u8()? != 0,
                exchange_index: cursor.read_u8()?,
                item_sprite: {
                    let raw = cursor.read_u16::<BigEndian>()?;
                    raw & 0x7FFF
                },
                item_color: cursor.read_u8()?,
                item_name: decode_string(&mut cursor, "item_name")?,
            }),
            3 => Ok(DisplayExchange::SetGold {
                right_side: cursor.read_u8()? != 0,
                gold_amount: cursor.read_u32::<BigEndian>()?,
            }),
            4 => Ok(DisplayExchange::Cancel {
                right_side: cursor.read_u8()? != 0,
                message: decode_string(&mut cursor, "message")?,
            }),
            5 => Ok(DisplayExchange::Accept {
                right_side: cursor.read_u8()? != 0,
                message: decode_string(&mut cursor, "message")?,
            }),
            other => bail!("Invalid exchange response action: {other}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_exchange_round_trip() {
        let packets = vec![
            DisplayExchange::Start {
                other_user_id: 12345,
                other_user_name: "Aisling".to_string(),
            },
            DisplayExchange::RequestAmount { from_slot: 2 },
            DisplayExchange::AddItem {
                right_side: true,
                exchange_index: 1,
                item_sprite: 42,
                item_color: 3,
                item_name: "Hy-brasyl Sword".to_string(),
            },
            DisplayExchange::SetGold {
                right_side: false,
                gold_amount: 100000,
            },
            DisplayExchange::Cancel {
                right_side: true,
                message: "Trade cancelled".to_string(),
            },
            DisplayExchange::Accept {
                right_side: true,
                message: "Accepted".to_string(),
            },
        ];

        for pkt in packets {
            let mut bytes = Vec::new();
            pkt.write_payload(&mut bytes);
            let parsed = DisplayExchange::try_from_bytes(&bytes).unwrap();
            assert_eq!(pkt, parsed);
        }
    }
}
