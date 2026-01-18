use crate::TryFromBytes;
use anyhow::anyhow;
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::io::{Cursor, Read};

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
enum ExchangeResponseType {
    StartExchange = 0,
    RequestAmount = 1,
    AddItem = 2,
    SetGold = 3,
    Cancel = 4,
    Accept = 5,
}

#[derive(Debug, Clone)]
pub enum DisplayExchange {
    StartExchange {
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
        gold_amount: i32,
    },
    Cancel {
        right_side: bool,
        message: String,
    },
    Accept {
        persist_exchange: bool,
        message: String,
    },
}

impl TryFromBytes for DisplayExchange {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let exchange_response_type: ExchangeResponseType = cursor.read_u8()?.try_into()?;

        let decode_string = |cursor: &mut Cursor<&[u8]>, label: &str| -> anyhow::Result<String> {
            let len = cursor.read_u8()? as usize;
            let mut buf = vec![0; len];
            cursor.read_exact(&mut buf)?;
            WINDOWS_949
                .decode(&buf, DecoderTrap::Replace)
                .map_err(|e| anyhow!("Failed to decode {}: {}", label, e))
        };

        Ok(match exchange_response_type {
            ExchangeResponseType::StartExchange => DisplayExchange::StartExchange {
                other_user_id: cursor.read_u32::<BigEndian>()?,
                other_user_name: decode_string(&mut cursor, "other_user_name")?,
            },
            ExchangeResponseType::RequestAmount => DisplayExchange::RequestAmount {
                from_slot: cursor.read_u8()?,
            },
            ExchangeResponseType::AddItem => DisplayExchange::AddItem {
                right_side: cursor.read_u8()? != 0,
                exchange_index: cursor.read_u8()?,
                item_sprite: cursor.read_u16::<BigEndian>()?,
                item_color: cursor.read_u8()?,
                item_name: decode_string(&mut cursor, "item_name")?,
            },
            ExchangeResponseType::SetGold => DisplayExchange::SetGold {
                right_side: cursor.read_u8()? != 0,
                gold_amount: cursor.read_i32::<BigEndian>()?,
            },
            ExchangeResponseType::Cancel => DisplayExchange::Cancel {
                right_side: cursor.read_u8()? != 0,
                message: decode_string(&mut cursor, "message")?,
            },
            ExchangeResponseType::Accept => DisplayExchange::Accept {
                persist_exchange: cursor.read_u8()? != 0,
                message: decode_string(&mut cursor, "message")?,
            },
        })
    }
}
