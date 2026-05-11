use byteorder::{LE, ReadBytesExt};
use circbuf::CircBuf;
use std::io::{self, Read, Write};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DatEntryMetadata {
    pub(crate) name: String,
    pub(crate) size: usize,
}

pub(crate) fn read_dat_entries<F>(
    decoder: &mut dyn Read,
    on_entry: &mut F,
) -> anyhow::Result<()>
where
    F: FnMut(DatEntryMetadata, &mut dyn Read) -> anyhow::Result<()>,
{
    let mut reader = DatReader::new(decoder)?;

    while let Some(entry) = reader.next_entry()? {
        let metadata = entry.metadata.clone();
        let mut entry_reader = DatEntryReader {
            reader: &mut reader,
            remaining: entry.metadata.size,
        };

        on_entry(metadata, &mut entry_reader)?;
        io::copy(&mut entry_reader, &mut io::sink())?;
    }

    Ok(())
}

struct DatReader<'a> {
    decoder: &'a mut dyn Read,
    dat_buffer: CircBuf,
    scratch: Vec<u8>,
    entries: Vec<DatEntryMetadata>,
    catalog_loaded: bool,
}

impl<'a> DatReader<'a> {
    fn new(decoder: &'a mut dyn Read) -> anyhow::Result<Self> {
        Ok(Self {
            decoder,
            dat_buffer: CircBuf::with_capacity(8192)?,
            scratch: vec![0u8; 4096],
            entries: Vec::new(),
            catalog_loaded: false,
        })
    }

    fn next_entry(&mut self) -> anyhow::Result<Option<DatEntryHandle>> {
        self.load_catalog()?;

        Ok(self
            .entries
            .pop()
            .map(|metadata| DatEntryHandle { metadata }))
    }

    fn load_catalog(&mut self) -> anyhow::Result<()> {
        if self.catalog_loaded {
            return Ok(());
        }

        if !self.fill_until(4)? {
            self.catalog_loaded = true;
            return Ok(());
        }

        let file_count = self.dat_buffer.read_u32::<LE>()?;
        let catalog_bytes = file_count as usize * 17;
        anyhow::ensure!(
            self.fill_until(catalog_bytes)?,
            "unexpected EOF while reading DAT file table"
        );

        for index in 0..file_count {
            let offset = self.dat_buffer.read_u32::<LE>()?;

            let mut name_buf = [0u8; 13];
            self.dat_buffer.read_exact(&mut name_buf)?;
            let null_index = memchr::memchr(b'\0', &name_buf).unwrap_or(13);

            if null_index == 0 {
                continue;
            }

            let name = String::from_utf8_lossy(&name_buf[..null_index])
                .trim_end()
                .to_lowercase();

            if name.is_empty() {
                continue;
            }

            let is_last_file = index == (file_count - 1);
            let size = if is_last_file {
                0
            } else {
                let next_offset = self.dat_buffer.reader_peek().read_u32::<LE>()?;
                (next_offset - offset) as usize
            };

            self.entries.push(DatEntryMetadata { name, size });
        }

        self.entries.reverse();
        self.catalog_loaded = true;
        Ok(())
    }

    fn fill_until(&mut self, min_len: usize) -> anyhow::Result<bool> {
        while self.dat_buffer.len() < min_len {
            let bytes_read = self.decoder.read(&mut self.scratch)?;
            if bytes_read == 0 {
                return Ok(false);
            }

            while bytes_read > self.dat_buffer.avail() {
                self.dat_buffer.grow()?;
            }
            self.dat_buffer.write(&self.scratch[..bytes_read])?;
        }

        Ok(true)
    }

    fn read_entry_chunk(&mut self, buf: &mut [u8], remaining: &mut usize) -> anyhow::Result<usize> {
        if *remaining == 0 || buf.is_empty() {
            return Ok(0);
        }

        if self.dat_buffer.is_empty() {
            anyhow::ensure!(
                self.fill_until(1)?,
                "unexpected EOF while reading DAT entry content"
            );
        }

        let to_read = buf.len().min(*remaining).min(self.dat_buffer.len());
        self.dat_buffer.read_exact(&mut buf[..to_read])?;
        *remaining -= to_read;

        Ok(to_read)
    }
}

struct DatEntryHandle {
    metadata: DatEntryMetadata,
}

struct DatEntryReader<'reader, 'decoder> {
    reader: &'reader mut DatReader<'decoder>,
    remaining: usize,
}

impl Read for DatEntryReader<'_, '_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reader
            .read_entry_chunk(buf, &mut self.remaining)
            .map_err(io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::{read_dat_entries, DatEntryMetadata};
    use byteorder::{LE, WriteBytesExt};
    use std::io::Cursor;

    #[test]
    fn read_dat_entries_yields_ordered_streams() {
        let mut bytes = Vec::new();
        bytes.write_u32::<LE>(3).unwrap();
        write_catalog_entry(&mut bytes, 0, "one.bin");
        write_catalog_entry(&mut bytes, 4, "two.bin");
        write_catalog_entry(&mut bytes, 7, "three.bin");
        bytes.extend_from_slice(b"ABCDXYZ");

        let mut reader = Cursor::new(bytes);
        let mut seen = Vec::new();

        read_dat_entries(&mut reader, &mut |metadata, contents| {
            let mut buf = Vec::new();
            contents.read_to_end(&mut buf)?;
            seen.push((metadata, buf));
            Ok(())
        })
        .unwrap();

        assert_eq!(
            seen,
            vec![
                (
                    DatEntryMetadata {
                        name: "one.bin".to_string(),
                        size: 4,
                    },
                    b"ABCD".to_vec(),
                ),
                (
                    DatEntryMetadata {
                        name: "two.bin".to_string(),
                        size: 3,
                    },
                    b"XYZ".to_vec(),
                ),
                (
                    DatEntryMetadata {
                        name: "three.bin".to_string(),
                        size: 0,
                    },
                    Vec::new(),
                ),
            ]
        );
    }

    #[test]
    fn read_dat_entries_skips_empty_names() {
        let mut bytes = Vec::new();
        bytes.write_u32::<LE>(2).unwrap();
        write_catalog_entry(&mut bytes, 0, "");
        write_catalog_entry(&mut bytes, 2, "kept.bin");
        bytes.extend_from_slice(b"HI");

        let mut reader = Cursor::new(bytes);
        let mut seen = Vec::new();

        read_dat_entries(&mut reader, &mut |metadata, contents| {
            let mut buf = Vec::new();
            contents.read_to_end(&mut buf)?;
            seen.push((metadata.name, metadata.size, buf));
            Ok(())
        })
        .unwrap();

        assert_eq!(seen, vec![("kept.bin".to_string(), 0, Vec::new())]);
    }

    fn write_catalog_entry(bytes: &mut Vec<u8>, offset: u32, name: &str) {
        bytes.write_u32::<LE>(offset).unwrap();

        let mut name_buf = [0u8; 13];
        let source = name.as_bytes();
        let len = source.len().min(12);
        name_buf[..len].copy_from_slice(&source[..len]);
        bytes.extend_from_slice(&name_buf);
    }
}