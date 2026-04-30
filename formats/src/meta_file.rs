use byteorder::{BigEndian, ReadBytesExt};
use encoding::all::WINDOWS_949;
use encoding::{DecoderTrap, Encoding};
use std::io::{Cursor, Read};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaFileEntry {
    pub name: String,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct MetaFile {
    pub entries: Vec<MetaFileEntry>,
}

impl MetaFile {
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);
        if bytes.is_empty() {
            return Ok(MetaFile::default());
        }

        let entry_count = cursor.read_u16::<BigEndian>()?;
        let mut entries = Vec::with_capacity(entry_count as usize);

        for _ in 0..entry_count {
            let name_len = cursor.read_u8()? as usize;
            let mut name_buf = vec![0; name_len];
            cursor.read_exact(&mut name_buf)?;
            let name = WINDOWS_949
                .decode(&name_buf, DecoderTrap::Replace)
                .unwrap_or_default();

            let prop_count = cursor.read_u16::<BigEndian>()?;
            let mut fields = Vec::with_capacity(prop_count as usize);

            for _ in 0..prop_count {
                let val_len = cursor.read_u16::<BigEndian>()? as usize;
                let mut val_buf = vec![0; val_len];
                cursor.read_exact(&mut val_buf)?;
                let value = WINDOWS_949
                    .decode(&val_buf, DecoderTrap::Replace)
                    .unwrap_or_default();
                fields.push(value);
            }

            entries.push(MetaFileEntry { name, fields });
        }

        Ok(MetaFile { entries })
    }
}
