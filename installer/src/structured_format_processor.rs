use byteorder::{LE, ReadBytesExt};
use formats::efa::EfaFile;
use formats::{
    epf::{EpfFrame, EpfImage},
    mpf::MpfFile,
};
use std::io::{Cursor, Read};
use std::path::Path;

use crate::asset_record::AssetRecord;
use crate::deferred_job::EffectAssetJobBuilder;
use crate::sheet_processor::{self, sheet_records};

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
        effect_job: &mut EffectAssetJobBuilder,
    ) -> anyhow::Result<StructuredDatEntry> {
        if file_name.ends_with(".mpf") {
            let mut file_buffer = vec![0u8; file_size];
            entry_reader.read_exact(&mut file_buffer)?;

            let mut reader = Cursor::new(file_buffer);
            let mpf = MpfFile::read_from_da(&mut reader).expect("Failed to read MPF file");
            let base = file_name.trim_end_matches(".mpf");
            let (sheet, sheet_images) = sheet_processor::build_creature_sheets(&mpf);
            let sheet_bytes = oxicode::encode_to_vec(&sheet)?;

            let records = sheet_records(
                dat_path,
                base,
                1, // palette-indexed R8 pixels
                sheet_bytes,
                sheet.chunks,
                sheet_images,
            )?;

            return Ok(StructuredDatEntry::Assets(records));
        }

        if file_name.ends_with(".efa") {
            let mut file_buffer = vec![0u8; file_size];
            entry_reader.read_exact(&mut file_buffer)?;

            let mut reader = Cursor::new(file_buffer);
            let is_effect = dat_path
                .file_name()
                .map(|name| name.eq_ignore_ascii_case("roh"))
                .unwrap_or(false)
                && file_name.starts_with("efct");
            return match EfaFile::read_from_da(&mut reader) {
                Ok(efa) => {
                    if is_effect {
                        let effect_id = file_name[4..7].parse::<u16>().unwrap_or(0);
                        effect_job.push_efa(effect_id, efa);
                        Ok(StructuredDatEntry::Assets(Vec::new()))
                    } else {
                        let efa_bytes = oxicode::encode_to_vec(&efa)?;
                        Ok(StructuredDatEntry::Assets(vec![AssetRecord::bytes(
                            dat_path.join(file_name.replace(".efa", ".efa.bin")),
                            efa_bytes,
                        )]))
                    }
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

            let is_item_sheet = dat_path
                .file_name()
                .map(|name| name.eq_ignore_ascii_case("Legend"))
                .unwrap_or(false)
                && file_name.starts_with("item");
            let is_effect = dat_path
                .file_name()
                .map(|name| name.eq_ignore_ascii_case("roh"))
                .unwrap_or(false)
                && file_name.starts_with("efct");

            if is_effect {
                let effect_id = file_name[4..7].parse::<u16>().unwrap_or(0);
                effect_job.push_epf(effect_id, epf);
                return Ok(StructuredDatEntry::Assets(Vec::new()));
            }

            if is_item_sheet {
                let base = file_name.trim_end_matches(".epf");
                let (sheet, sheet_images) = sheet_processor::build_item_sheets(&epf);
                let sheet_bytes = oxicode::encode_to_vec(&sheet)?;
                let records = sheet_records(
                    dat_path,
                    base,
                    1, // palette-indexed R8 pixels
                    sheet_bytes,
                    sheet.chunks,
                    sheet_images,
                )?;
                return Ok(StructuredDatEntry::Assets(records));
            }

            // Every other EPF (UI icon sets like `setoa/skill001`, world map
            // images, leftover Legend parts) is packed the same way, so the
            // runtime always reads one shared `{base}.sheet.bin` file and
            // never stores the pixel payload twice.
            let base = file_name.trim_end_matches(".epf");
            let (sheet, sheet_images) = sheet_processor::build_item_sheets(&epf);
            let sheet_bytes = oxicode::encode_to_vec(&sheet)?;
            return Ok(StructuredDatEntry::Assets(sheet_records(
                dat_path,
                base,
                1, // palette-indexed R8 pixels
                sheet_bytes,
                sheet.chunks,
                sheet_images,
            )?));
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
            frames.push(EpfFrame::new_empty());
            continue;
        }

        frames.push(EpfFrame::new(
            top as u16,
            left as u16,
            bottom as u16,
            right as u16,
            file_buffer[start_address..(start_address + bytes_to_read)].to_vec(),
        ));
    }

    Ok(EpfImage {
        width: pixel_width as u16,
        height: pixel_height as u16,
        frames,
    })
}

#[cfg(test)]
mod tests {
    use super::{StructuredDatEntry, StructuredFormatProcessor};
    use crate::asset_record::AssetRecord;
    use std::io::Cursor;
    use std::path::Path;

    fn test_epf_buffer() -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&1u16.to_le_bytes()); // frame_count
        buf.extend_from_slice(&32u16.to_le_bytes()); // pixel_width
        buf.extend_from_slice(&32u16.to_le_bytes()); // pixel_height
        buf.extend_from_slice(&0u16.to_le_bytes()); // unknown
        buf.extend_from_slice(&0u32.to_le_bytes()); // toc_address
        buf.extend_from_slice(&0u16.to_le_bytes()); // top
        buf.extend_from_slice(&0u16.to_le_bytes()); // left
        buf.extend_from_slice(&16u16.to_le_bytes()); // bottom
        buf.extend_from_slice(&16u16.to_le_bytes()); // right
        buf.extend_from_slice(&0u32.to_le_bytes()); // start_address
        buf.extend_from_slice(&256u32.to_le_bytes()); // end_address
        buf.extend_from_slice(&[7u8; 256]); // palette indices
        buf
    }

    fn test_mpf_buffer() -> Vec<u8> {
        let mut buf = Vec::new();
        // The header doubles as the first fields: the initial i32 is
        // reinterpreted as frame_count | pixel_width<<8 | pixel_height<<16 |
        // data_length_byte<<24, and is only a 12-byte header when it equals -1.
        buf.push(1); // frame_count
        buf.extend_from_slice(&1i16.to_le_bytes()); // pixel_width (bytes 1..3)
        buf.extend_from_slice(&1i16.to_le_bytes()); // pixel_height (bytes 3..5)
        buf.extend_from_slice(&256i32.to_le_bytes()); // data_length
        buf.push(0); // walk frame_index_away
        buf.push(1); // walk frame_count
        buf.extend_from_slice(&0i16.to_le_bytes()); // has_multiple_attacks = false
        // The parser re-reads the two bytes of `has_multiple_attacks` as the
        // attack animation's fields, so only standing/optional/extra follow.
        buf.push(0); // standing frame_index_away
        buf.push(0); // standing frame_count
        buf.push(0); // optional_frame_count
        buf.push(0); // extra animation type
        buf.extend_from_slice(&0i16.to_le_bytes()); // left
        buf.extend_from_slice(&0i16.to_le_bytes()); // top
        buf.extend_from_slice(&16i16.to_le_bytes()); // right
        buf.extend_from_slice(&16i16.to_le_bytes()); // bottom
        buf.extend_from_slice(&8i16.to_le_bytes()); // center_x
        buf.extend_from_slice(&16i16.to_le_bytes()); // center_y
        buf.extend_from_slice(&0i32.to_le_bytes()); // start_address
        buf.extend_from_slice(&[7u8; 256]); // palette indices
        buf
    }

    fn emitted_records(
        processor: &StructuredFormatProcessor,
        dat_path: &Path,
        file_name: &str,
        bytes: Vec<u8>,
    ) -> Vec<AssetRecord> {
        let mut effect_job = crate::deferred_job::EffectAssetJobBuilder::new();
        let mut reader = Cursor::new(bytes);
        match processor
            .process_entry(
                dat_path,
                file_name,
                reader.get_ref().len(),
                &mut reader,
                false,
                &mut effect_job,
            )
            .unwrap()
        {
            StructuredDatEntry::Assets(records) => records,
            _ => panic!("expected asset records"),
        }
    }

    fn emitted_paths(records: &[AssetRecord]) -> Vec<String> {
        records
            .iter()
            .map(|record| record.path().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn process_entry_ignores_unstructured_files() {
        let processor = StructuredFormatProcessor;
        let mut effect_job = crate::deferred_job::EffectAssetJobBuilder::new();
        let mut reader = Cursor::new(Vec::<u8>::new());

        let result = processor
            .process_entry(
                Path::new("Legend"),
                "foo.txt",
                0,
                &mut reader,
                false,
                &mut effect_job,
            )
            .unwrap();

        assert!(matches!(result, StructuredDatEntry::Unhandled));
    }

    #[test]
    fn mpf_emits_sheet_records_without_duplicate_pixel_payload() {
        let processor = StructuredFormatProcessor;
        let records = emitted_records(&processor, Path::new("hades"), "mns001.mpf", test_mpf_buffer());
        let paths = emitted_paths(&records);

        assert!(paths.contains(&"hades/mns001.sheet.bin".to_string()));
        assert_eq!(records.len(), 1);
        assert!(!paths.iter().any(|path| path.ends_with(".mpf.bin")));
    }

    #[test]
    fn item_epf_emits_sheet_records_without_duplicate_pixel_payload() {
        let processor = StructuredFormatProcessor;
        let records = emitted_records(&processor, Path::new("Legend"), "item001.epf", test_epf_buffer());
        let paths = emitted_paths(&records);

        assert!(paths.contains(&"Legend/item001.sheet.bin".to_string()));
        assert_eq!(records.len(), 1);
        assert!(!paths.iter().any(|path| path.ends_with(".epf.bin")));
    }

    #[test]
    fn plain_epf_emits_sheet_records_for_ui_consumers() {
        let processor = StructuredFormatProcessor;
        let records = emitted_records(&processor, Path::new("setoa"), "skill001.epf", test_epf_buffer());
        let paths = emitted_paths(&records);

        assert!(paths.contains(&"setoa/skill001.sheet.bin".to_string()));
        assert_eq!(records.len(), 1);
        assert!(!paths.iter().any(|path| path.ends_with(".epf.bin")));
    }
}
