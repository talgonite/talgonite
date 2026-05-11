use anyhow::Context;
use byteorder::{LE, ReadBytesExt};
use crc32fast::Hasher;
use flate2::bufread::DeflateDecoder;
use std::io::{self, BufRead, BufReader, Cursor, Read};

use crate::source::SourceReader;

const HEADER_SIZE_TO_SKIP: u64 = 1024 * 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PayloadKind {
    Dat,
    Music,
    Other,
}

impl PayloadKind {
    fn from_path(path: &str) -> Self {
        if path.ends_with(".dat") {
            Self::Dat
        } else if path.ends_with(".mus") {
            Self::Music
        } else {
            Self::Other
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PayloadEntry {
    pub(crate) file_path: String,
    pub(crate) compressed_size: u64,
    pub(crate) kind: PayloadKind,
    deflate_start: u32,
    expected_crc32: u32,
}

impl PayloadEntry {
    fn from_file_header(file_header: FileHeader) -> anyhow::Result<Self> {
        let compressed_size = file_header
            .deflate_end
            .checked_sub(file_header.deflate_start)
            .and_then(|span| span.checked_sub(4))
            .with_context(|| {
                format!(
                    "invalid deflate span for {}: {}..{}",
                    file_header.file_path, file_header.deflate_start, file_header.deflate_end
                )
            })?;

        Ok(Self {
            kind: PayloadKind::from_path(&file_header.file_path),
            file_path: file_header.file_path,
            compressed_size: u64::from(compressed_size),
            deflate_start: file_header.deflate_start,
            expected_crc32: file_header.crc32,
        })
    }
}

pub(crate) struct Da741ExeReader<R: Read> {
    reader: BufReader<R>,
    payloads: Vec<PayloadEntry>,
    file_data_start: u64,
    reader_position: u64,
}

impl Da741ExeReader<SourceReader> {
    pub(crate) fn from_source(source: SourceReader) -> anyhow::Result<Self> {
        let executable_offset = source.executable_offset();
        Self::new_with_offset(source, executable_offset)
    }
}

impl<R: Read> Da741ExeReader<R> {
    fn new_with_offset(source: R, executable_offset: u64) -> anyhow::Result<Self> {
        let mut reader = BufReader::new(source);
        let (payloads, file_data_start) = parse_payload_catalog(&mut reader, executable_offset)?;

        Ok(Self {
            reader,
            payloads,
            file_data_start,
            reader_position: HEADER_SIZE_TO_SKIP,
        })
    }

    pub(crate) fn payloads(&self) -> &[PayloadEntry] {
        &self.payloads
    }

    pub(crate) fn read_payload<T, F>(
        &mut self,
        payload: &PayloadEntry,
        read_payload: F,
    ) -> anyhow::Result<T>
    where
        F: FnOnce(&mut dyn Read) -> anyhow::Result<T>,
    {
        let data_start = self.file_data_start + u64::from(payload.deflate_start);
        anyhow::ensure!(
            self.reader_position <= data_start,
            "payload traversal moved backwards for {}",
            payload.file_path
        );

        self.reader
            .seek_relative((data_start - self.reader_position) as i64)?;
        self.reader_position = data_start;

        let (result, hash) = {
            let mut file_reader = (&mut self.reader).take(payload.compressed_size);
            let mut hashing_reader = HashingReader::new(DeflateDecoder::new(&mut file_reader));

            let result = read_payload(&mut hashing_reader)?;
            io::copy(&mut hashing_reader, &mut io::sink())?;
            let hash = hashing_reader.finalize();

            io::copy(&mut file_reader, &mut io::sink())?;

            (result, hash)
        };

        anyhow::ensure!(
            payload.expected_crc32 == hash,
            "CRC32 mismatch for {}",
            payload.file_path
        );

        let mut crc32_buffer = [0u8; 4];
        self.reader.read_exact(&mut crc32_buffer)?;
        self.reader_position += payload.compressed_size + 4;

        let stored_crc32 = u32::from_le_bytes(crc32_buffer);
        anyhow::ensure!(
            stored_crc32 == hash,
            "CRC32 mismatch for {}",
            payload.file_path
        );

        Ok(result)
    }
}

fn parse_payload_catalog<R: Read>(
    exe_reader: &mut BufReader<R>,
    executable_offset: u64,
) -> anyhow::Result<(Vec<PayloadEntry>, u64)> {
    let mut header_reader = (&mut *exe_reader).take(HEADER_SIZE_TO_SKIP);

    header_reader.seek_relative(executable_offset as i64)?;
    let wise_header = read_wise_overlay_header(&mut header_reader)?;

    header_reader.seek_relative(i64::from(wise_header.dib_compressed_size))?;

    let mut script = Vec::with_capacity(wise_header.wise_script_uncompressed_size as usize);
    DeflateDecoder::new(&mut header_reader).read_to_end(&mut script)?;

    let _script_crc32 = header_reader.read_u32::<LE>()?;
    io::copy(&mut header_reader, &mut io::sink())?;

    let mut reader = Cursor::new(script);
    read_header(&mut reader)?;
    read_languages(&mut reader)?;

    let mut operations = Vec::new();
    loop {
        match read_operation(&mut reader) {
            Ok(operation) => operations.push(operation),
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error.into()),
        }
    }

    build_payload_catalog(operations, wise_header.eof)
}

fn build_payload_catalog(
    operations: Vec<Operation>,
    eof: u32,
) -> anyhow::Result<(Vec<PayloadEntry>, u64)> {
    let last_deflate_end = operations
        .iter()
        .map(|operation| match operation {
            Operation::CreateFile(file_header) => file_header.deflate_end,
            Operation::UnknownFile(deflate_end) => *deflate_end,
            Operation::NoOp => 0,
        })
        .max()
        .context("no WISE payload operations found")?;

    let file_data_start = eof
        .checked_sub(last_deflate_end)
        .context("WISE payload data begins past EOF")?;

    let payloads = operations
        .into_iter()
        .filter_map(|operation| match operation {
            Operation::CreateFile(file_header) => Some(PayloadEntry::from_file_header(file_header)),
            Operation::NoOp | Operation::UnknownFile(_) => None,
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok((payloads, u64::from(file_data_start)))
}

fn read_header<R: Read + ?Sized + BufRead>(reader: &mut R) -> io::Result<()> {
    reader.seek_relative(43)
}

fn read_languages<R: Read + ?Sized + BufRead>(reader: &mut R) -> io::Result<()> {
    reader.skip_until(0)?;
    reader.skip_until(0)?;
    reader.skip_until(0)?;

    reader.seek_relative(6)?;

    if reader.read_u8()? != 0x01 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected WISE language marker",
        ));
    }

    reader.seek_relative(7)?;

    for _ in 0..56 {
        reader.skip_until(0)?;
    }

    Ok(())
}

fn read_operation<R: Read + ?Sized + BufRead>(reader: &mut R) -> io::Result<Operation> {
    let id = reader.read_u8()?;
    match id {
        0x00 => Ok(Operation::CreateFile(read_file_header(reader)?)),
        0x03 => {
            reader.seek_relative(1)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x04 => {
            reader.seek_relative(1)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x05 => {
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x07 => {
            reader.seek_relative(1)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x08 => {
            reader.seek_relative(1)?;
            Ok(Operation::NoOp)
        }
        0x09 => {
            reader.seek_relative(1)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x0a => {
            reader.seek_relative(2)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x0b => {
            reader.seek_relative(1)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x0c => {
            reader.seek_relative(1)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x0d | 0x0f | 0x10 | 0x1b => Ok(Operation::NoOp),
        0x11 => {
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x14 => {
            reader.seek_relative(4)?;
            let deflate_end = reader.read_u32::<LE>()?;
            reader.seek_relative(4)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            Ok(Operation::UnknownFile(deflate_end))
        }
        0x15 => {
            reader.seek_relative(1)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x16 => {
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x18 => {
            if reader.read_u8()? != 0x1b {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unexpected WISE 0x18 marker",
                ));
            }
            Ok(Operation::NoOp)
        }
        0x1c => {
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x1e => {
            reader.seek_relative(1)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown WISE operation: 0x{id:02X}"),
        )),
    }
}

#[derive(Debug)]
enum Operation {
    NoOp,
    CreateFile(FileHeader),
    UnknownFile(u32),
}

#[derive(Debug)]
struct FileHeader {
    deflate_start: u32,
    deflate_end: u32,
    crc32: u32,
    file_path: String,
}

fn read_file_header<R: Read + ?Sized + BufRead>(reader: &mut R) -> io::Result<FileHeader> {
    reader.seek_relative(2)?;
    let deflate_start = reader.read_u32::<LE>()?;
    let deflate_end = reader.read_u32::<LE>()?;
    reader.seek_relative(28)?;
    let crc32 = reader.read_u32::<LE>()?;
    let file_path = read_null_terminated_string(reader)?
        .replace('\\', "/")
        .replace("%MAINDIR%/", "");
    reader.skip_until(0)?;
    reader.skip_until(0)?;

    Ok(FileHeader {
        deflate_start,
        deflate_end,
        crc32,
        file_path,
    })
}

fn read_null_terminated_string<R: Read + ?Sized + BufRead>(reader: &mut R) -> io::Result<String> {
    let mut buffer = Vec::new();
    reader.read_until(0, &mut buffer)?;

    if buffer.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "expected a null-terminated string",
        ));
    }

    Ok(String::from_utf8_lossy(&buffer[..buffer.len() - 1]).to_string())
}

#[derive(Debug)]
struct WiseOverlayHeader {
    wise_script_uncompressed_size: u32,
    eof: u32,
    dib_compressed_size: u32,
}

fn read_wise_overlay_header<R: Read + ?Sized + BufRead>(
    reader: &mut R,
) -> io::Result<WiseOverlayHeader> {
    if reader.read_u8()? != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected WISE overlay marker",
        ));
    }

    reader.seek_relative(24)?;
    let wise_script_uncompressed_size = reader.read_u32::<LE>()?;
    reader.seek_relative(48)?;
    let eof = reader.read_u32::<LE>()?;
    let dib_compressed_size = reader.read_u32::<LE>()?;
    reader.seek_relative(6)?;
    let init_text_length = reader.read_u8()?;
    reader.seek_relative(i64::from(init_text_length))?;

    Ok(WiseOverlayHeader {
        wise_script_uncompressed_size,
        eof,
        dib_compressed_size,
    })
}

trait SeekExt {
    fn seek_relative(&mut self, offset: i64) -> io::Result<()>;
}

impl<T: Read + ?Sized> SeekExt for T {
    fn seek_relative(&mut self, offset: i64) -> io::Result<()> {
        let offset = u64::try_from(offset)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "offset is negative"))?;

        io::copy(&mut self.take(offset), &mut io::sink())?;
        Ok(())
    }
}

struct HashingReader<R> {
    reader: R,
    hasher: Hasher,
}

impl<R> HashingReader<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            hasher: Hasher::new(),
        }
    }

    fn finalize(self) -> u32 {
        self.hasher.finalize()
    }
}

impl<R: Read> Read for HashingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let bytes_read = self.reader.read(buf)?;
        self.hasher.update(&buf[..bytes_read]);
        Ok(bytes_read)
    }
}

#[cfg(test)]
mod tests {
    use super::{FileHeader, Operation, PayloadKind, build_payload_catalog, read_file_header};
    use byteorder::{LE, WriteBytesExt};
    use std::io::Cursor;

    #[test]
    fn read_file_header_normalizes_main_dir_paths() {
        let mut bytes = vec![0u8; 2];
        bytes.write_u32::<LE>(10).unwrap();
        bytes.write_u32::<LE>(42).unwrap();
        bytes.extend([0u8; 28]);
        bytes.write_u32::<LE>(0x1234_5678).unwrap();
        bytes.extend(b"%MAINDIR%\\Legend.dat\0\0\0");

        let mut reader = Cursor::new(bytes);
        let file_header = read_file_header(&mut reader).unwrap();

        assert_eq!(file_header.deflate_start, 10);
        assert_eq!(file_header.deflate_end, 42);
        assert_eq!(file_header.crc32, 0x1234_5678);
        assert_eq!(file_header.file_path, "Legend.dat");
    }

    #[test]
    fn build_payload_catalog_computes_data_start_and_payload_kinds() {
        let operations = vec![
            Operation::NoOp,
            Operation::CreateFile(FileHeader {
                deflate_start: 10,
                deflate_end: 30,
                crc32: 1,
                file_path: "Legend.dat".to_string(),
            }),
            Operation::UnknownFile(50),
            Operation::CreateFile(FileHeader {
                deflate_start: 60,
                deflate_end: 90,
                crc32: 2,
                file_path: "music/title.mus".to_string(),
            }),
            Operation::CreateFile(FileHeader {
                deflate_start: 91,
                deflate_end: 120,
                crc32: 3,
                file_path: "readme.txt".to_string(),
            }),
        ];

        let (payloads, file_data_start) = build_payload_catalog(operations, 200).unwrap();

        assert_eq!(file_data_start, 80);
        assert_eq!(payloads.len(), 3);
        assert_eq!(payloads[0].kind, PayloadKind::Dat);
        assert_eq!(payloads[0].compressed_size, 16);
        assert_eq!(payloads[1].kind, PayloadKind::Music);
        assert_eq!(payloads[1].compressed_size, 26);
        assert_eq!(payloads[2].kind, PayloadKind::Other);
        assert_eq!(payloads[2].compressed_size, 25);
    }
}
