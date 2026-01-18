use crate::TryFromBytes;
use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use std::io::{Cursor, Read};

#[derive(Debug)]
pub struct MetaDataChecksum {
    pub name: String,
    pub check_sum: u32,
}

#[derive(Debug)]
pub enum MetaData {
    DataByName {
        name: String,
        check_sum: u32,
        data: Vec<u8>,
    },
    AllCheckSums {
        collection: Vec<MetaDataChecksum>,
    },
}

impl TryFromBytes for MetaData {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        let type_ = cursor.read_u8()?;

        match type_ {
            0 => {
                let name = {
                    let mut buf = vec![0; cursor.read_u8()? as usize];
                    cursor.read_exact(&mut buf)?;
                    WINDOWS_949
                        .decode(&buf, DecoderTrap::Replace)
                        .unwrap_or_default()
                };
                let check_sum = cursor.read_u32::<BigEndian>()?;
                let data = {
                    let mut buf = vec![0; cursor.read_u16::<BigEndian>()? as usize];
                    cursor.read_exact(&mut buf)?;
                    buf
                };
                Ok(MetaData::DataByName {
                    name,
                    check_sum,
                    data,
                })
            }
            1 => {
                let count = cursor.read_u16::<BigEndian>()?;
                let mut collection = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    let name = {
                        let mut buf = vec![0; cursor.read_u8()? as usize];
                        cursor.read_exact(&mut buf)?;
                        WINDOWS_949
                            .decode(&buf, DecoderTrap::Replace)
                            .unwrap_or_default()
                    };
                    let check_sum = cursor.read_u32::<BigEndian>()?;
                    collection.push(MetaDataChecksum { name, check_sum });
                }
                Ok(MetaData::AllCheckSums { collection })
            }
            _ => anyhow::bail!("Unknown MetaData type: {}", type_),
        }
    }
}
