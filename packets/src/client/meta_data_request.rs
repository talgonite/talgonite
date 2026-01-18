use crate::ToBytes;

use super::Codes;
use encoding::all::WINDOWS_949;
use encoding::{EncoderTrap, Encoding};

#[derive(Debug)]
pub enum MetaDataRequest {
    DataByName(String),
    AllCheckSums,
}

impl ToBytes for MetaDataRequest {
    const OPCODE: u8 = Codes::MetaDataRequest as _;

    fn write_payload(&self, bytes: &mut Vec<u8>) {
        match self {
            MetaDataRequest::DataByName(name) => {
                bytes.push(0);
                let name_bytes = WINDOWS_949
                    .encode(&name, EncoderTrap::Replace)
                    .unwrap_or_default();
                bytes.push(name_bytes.len() as u8);
                bytes.extend_from_slice(&name_bytes);
            }
            MetaDataRequest::AllCheckSums => {
                bytes.push(1);
            }
        }
    }
}
