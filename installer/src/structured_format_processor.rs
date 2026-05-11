use byteorder::{LE, ReadBytesExt};
use formats::efa::EfaFile;
use formats::{
    epf::{EpfFrame, EpfImage},
    mpf::MpfFile,
};
use std::io::{Cursor, Read};
use std::path::Path;

use crate::asset_record::AssetRecord;

pub(crate) enum StructuredDatEntry {
    Unhandled,
    Assets(Vec<AssetRecord>),
    GroupedEpf { file_name: String, epf: EpfImage },
}

#[derive(Default)]
pub(crate) struct StructuredFormatProcessor;

impl StructuredFormatProcessor {
    pub(crate) fn process_entry(
        &self,
        dat_path: &Path,
        file_name: &str,
        file_size: usize,
        entry_reader: &mut dyn Read,
        group_epf: bool,
    ) -> anyhow::Result<StructuredDatEntry> {
        if file_name.ends_with(".mpf") {
            let mut file_buffer = vec![0u8; file_size];
            entry_reader.read_exact(&mut file_buffer)?;

            let mut reader = Cursor::new(file_buffer);
            let mpf = MpfFile::read_from_da(&mut reader).expect("Failed to read MPF file");
            let mpf_bytes = bincode::encode_to_vec(mpf, bincode::config::standard())?;

            return Ok(StructuredDatEntry::Assets(vec![AssetRecord::bytes(
                dat_path.join(file_name.replace(".mpf", ".mpf.bin")),
                mpf_bytes,
            )]));
        }

        if file_name.ends_with(".efa") {
            let mut file_buffer = vec![0u8; file_size];
            entry_reader.read_exact(&mut file_buffer)?;

            let mut reader = Cursor::new(file_buffer);
            return match EfaFile::read_from_da(&mut reader) {
                Ok(efa) => {
                    let efa_bytes = bincode::encode_to_vec(efa, bincode::config::standard())?;
                    Ok(StructuredDatEntry::Assets(vec![AssetRecord::bytes(
                        dat_path.join(file_name.replace(".efa", ".efa.bin")),
                        efa_bytes,
                    )]))
                }
                Err(error) => {
                    tracing::warn!("Failed to read EFA file {}: {:?}", file_name, error);
                    Ok(StructuredDatEntry::Assets(Vec::new()))
                }
            };
        }

        if file_name.ends_with(".epf") {
            let mut file_buffer = vec![0u8; file_size];
            entry_reader.read_exact(&mut file_buffer)?;
            let epf = read_epf(&file_buffer)?;

            if group_epf {
                return Ok(StructuredDatEntry::GroupedEpf {
                    file_name: file_name.to_string(),
                    epf,
                });
            }

            let epf_bytes = bincode::encode_to_vec(epf, bincode::config::standard())?;
            return Ok(StructuredDatEntry::Assets(vec![AssetRecord::bytes(
                dat_path.join(file_name.replace(".epf", ".epf.bin")),
                epf_bytes,
            )]));
        }

        Ok(StructuredDatEntry::Unhandled)
    }
}

fn read_epf(file_buffer: &[u8]) -> anyhow::Result<EpfImage> {
    let (frame_count, pixel_width, pixel_height, _, toc_address) = {
        let mut cursor = Cursor::new(file_buffer);

        (
            cursor.read_u16::<LE>()? as usize,
            cursor.read_u16::<LE>()? as usize,
            cursor.read_u16::<LE>()? as usize,
            cursor.read_u16::<LE>()?,
            cursor.read_u32::<LE>()? as usize,
        )
    };

    let file_buffer = &file_buffer[12..];
    let mut frames = Vec::with_capacity(frame_count);

    for i in 0..frame_count {
        let (top, left, bottom, right, start_address, _end_address) = {
            let mut cursor = Cursor::new(&file_buffer[(toc_address + i * 16)..]);

            (
                cursor.read_u16::<LE>()? as usize,
                cursor.read_u16::<LE>()? as usize,
                cursor.read_u16::<LE>()? as usize,
                cursor.read_u16::<LE>()? as usize,
                cursor.read_u32::<LE>()? as usize,
                cursor.read_u32::<LE>()? as usize,
            )
        };

        let width = right - left;
        let height = bottom - top;

        let bytes_to_read = width * height;
        let bytes_available = file_buffer.len() - start_address;

        if width == 0 || height == 0 || bytes_to_read > bytes_available {
            frames.push(EpfFrame::new(0, 0, 0, 0, vec![]));
            continue;
        }

        let data = file_buffer[start_address..(start_address + bytes_to_read)].to_vec();
        frames.push(EpfFrame::new(top, left, bottom, right, data));
    }

    Ok(EpfImage {
        width: pixel_width,
        height: pixel_height,
        frames,
    })
}

#[cfg(test)]
mod tests {
    use super::{StructuredDatEntry, StructuredFormatProcessor};
    use std::io::Cursor;
    use std::path::Path;

    #[test]
    fn process_entry_ignores_unstructured_files() {
        let processor = StructuredFormatProcessor;
        let mut reader = Cursor::new(Vec::<u8>::new());

        let result = processor
            .process_entry(Path::new("Legend"), "foo.txt", 0, &mut reader, false)
            .unwrap();

        assert!(matches!(result, StructuredDatEntry::Unhandled));
    }
}