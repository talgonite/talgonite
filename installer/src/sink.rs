use anyhow::Context;
use jubako::{self as jbk, creator::ContentAdder};
use libarx::{self as arx, CreatorError};
use std::{path::Path, rc::Rc, sync::Arc};

use crate::asset_record::AssetRecord;

pub(crate) trait AssetSink {
    fn write(&mut self, record: AssetRecord) -> anyhow::Result<()>;
}

pub(crate) struct ArxAssetSink {
    creator: libarx::create::SimpleCreator,
}

impl ArxAssetSink {
    pub(crate) fn new(output: &Path) -> anyhow::Result<Self> {
        let output_str = output
            .to_str()
            .context("installer output path must be valid UTF-8")?;

        let creator = libarx::create::SimpleCreator::new(
            jbk::Utf8Path::new(output_str),
            jbk::creator::ConcatMode::OneFile,
            Arc::new(()),
            Rc::new(()),
            jbk::creator::Compression::zstd(),
        )?;

        Ok(Self { creator })
    }

    pub(crate) fn finalize(self) -> anyhow::Result<()> {
        self.creator.finalize()?;
        Ok(())
    }
}

impl AssetSink for ArxAssetSink {
    fn write(&mut self, record: AssetRecord) -> anyhow::Result<()> {
        let entry = SimpleDataEntry::from_record(record, self.creator.adder())?;
        self.creator.add_entry(&entry)?;
        Ok(())
    }
}

struct SimpleDataEntry {
    path: arx::PathBuf,
    kind: arx::create::EntryKind,
}

impl SimpleDataEntry {
    fn from_record(
        record: AssetRecord,
        adder: &mut impl ContentAdder,
    ) -> anyhow::Result<Self> {
        let size = jbk::Size::new(record.size());
        let path = arx::PathBuf::from_path(record.path()).unwrap();
        let content_address =
            adder.add_content(record.into_input_reader(), jbk::creator::CompHint::Detect)?;
        Ok(Self {
            path,
            kind: arx::create::EntryKind::File(size, content_address),
        })
    }
}

impl arx::create::EntryTrait for SimpleDataEntry {
    fn kind(&self) -> Result<Option<arx::create::EntryKind>, CreatorError> {
        Ok(Some(self.kind.clone()))
    }

    fn path(&self) -> &arx::Path {
        &self.path
    }

    fn uid(&self) -> u64 {
        1000
    }

    fn gid(&self) -> u64 {
        1000
    }

    fn mode(&self) -> u64 {
        755
    }

    fn mtime(&self) -> u64 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::AssetSink;
    use crate::asset_record::AssetRecord;
    use std::path::{Path, PathBuf};

    #[derive(Default)]
    struct RecordingSink {
        entries: Vec<(PathBuf, Vec<u8>)>,
    }

    impl AssetSink for RecordingSink {
        fn write(&mut self, record: AssetRecord) -> anyhow::Result<()> {
            let mut bytes = vec![];
            let path = record.path().to_path_buf();
            let mut reader = record.into_input_reader();
            reader.read_to_end(&mut bytes)?;
            self.entries.push((path, bytes));
            Ok(())
        }
    }

    #[test]
    fn write_routes_asset_records_through_sink_contract() {
        let mut sink = RecordingSink::default();

        sink.write(AssetRecord::bytes(Path::new("Legend/file.bin"), b"abc".to_vec()))
            .unwrap();

        assert_eq!(
            sink.entries,
            vec![(PathBuf::from("Legend/file.bin"), b"abc".to_vec())]
        );
    }
}