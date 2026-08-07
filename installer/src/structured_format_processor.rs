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
            let mpf_bytes = oxicode::encode_to_vec(&mpf)?;
            let base = file_name.trim_end_matches(".mpf");
            let (sheet, sheet_images) = sheet_processor::build_creature_sheets(&mpf);
            let sheet_bytes = oxicode::encode_to_vec(&sheet)?;

            let mut records = vec![AssetRecord::bytes(
                dat_path.join(format!("{base}.mpf.bin")),
                mpf_bytes,
            )];
            records.extend(sheet_records(
                dat_path,
                base,
                formats::ktx2::VK_FORMAT_R8_UNORM,
                sheet_bytes,
                sheet.chunks,
                sheet_images,
            )?);

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
                let mut records = vec![AssetRecord::bytes(
                    dat_path.join(format!("{base}.epf.bin")),
                    oxicode::encode_to_vec(&epf)?,
                )];
                records.extend(sheet_records(
                    dat_path,
                    base,
                    formats::ktx2::VK_FORMAT_R8_UNORM,
                    sheet_bytes,
                    sheet.chunks,
                    sheet_images,
                )?);
                return Ok(StructuredDatEntry::Assets(records));
            }

            let epf_bytes = oxicode::encode_to_vec(&epf)?;
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
    use std::io::Cursor;
    use std::path::Path;

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
}
