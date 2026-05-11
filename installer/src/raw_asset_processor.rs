use std::io::Read;
use std::path::Path;

use crate::asset_record::AssetRecord;

#[derive(Default)]
pub(crate) struct RawAssetProcessor;

pub(crate) enum RawDatEntry {
    DeferredPalette { name: String, bytes: Vec<u8> },
    Asset(AssetRecord),
}

impl RawAssetProcessor {
    pub(crate) fn process_payload(
        &self,
        path: &Path,
        reader: &mut dyn Read,
    ) -> anyhow::Result<AssetRecord> {
        let mut bytes = vec![];
        reader.read_to_end(&mut bytes)?;
        Ok(AssetRecord::bytes(path.to_path_buf(), bytes))
    }

    pub(crate) fn process_dat_entry(
        &self,
        dat_path: &Path,
        file_name: String,
        bytes: Vec<u8>,
    ) -> RawDatEntry {
        if file_name.ends_with(".tbl") || file_name.ends_with(".pal") {
            RawDatEntry::DeferredPalette {
                name: file_name,
                bytes,
            }
        } else {
            RawDatEntry::Asset(AssetRecord::bytes(dat_path.join(file_name), bytes))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RawAssetProcessor, RawDatEntry};
    use std::path::Path;

    #[test]
    fn process_dat_entry_defers_palette_inputs() {
        let processor = RawAssetProcessor;

        let entry = processor.process_dat_entry(Path::new("Legend"), "item.tbl".to_string(), vec![1]);
        assert!(matches!(entry, RawDatEntry::DeferredPalette { .. }));
    }

    #[test]
    fn process_dat_entry_emits_passthrough_assets() {
        let processor = RawAssetProcessor;

        let entry = processor.process_dat_entry(Path::new("Legend"), "foo.txt".to_string(), vec![1]);
        assert!(matches!(entry, RawDatEntry::Asset(_)));
    }
}