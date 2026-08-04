use anyhow::Context;
use backhand::{
    FilesystemCompressor, FilesystemWriter, NodeHeader,
    compression::{CompressionOptions, Compressor, Zstd},
    kind,
    kind::Kind,
};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::asset_record::AssetRecord;

pub(crate) trait AssetSink {
    fn write(&mut self, record: AssetRecord) -> anyhow::Result<()>;
}

static SPOOL_COUNTER: AtomicU64 = AtomicU64::new(0);

/// SquashFS block size; 8 KiB keeps fragment reads cheap for small assets.
const ARCHIVE_BLOCK_SIZE: u32 = 8 * 1024;

/// Writes game assets into a SquashFS v4.0 little-endian image.
pub(crate) struct SquashfsAssetSink {
    writer: FilesystemWriter<'static, 'static, 'static>,
    spool: Arc<Mutex<File>>,
    spool_path: PathBuf,
    entries: Vec<(PathBuf, u64, u64)>,
    output: PathBuf,
}

impl SquashfsAssetSink {
    pub(crate) fn new(output: &Path) -> anyhow::Result<Self> {
        let mut writer = FilesystemWriter::default();
        writer.set_kind(Kind::from_const(kind::LE_V4_0).unwrap());
        writer.set_block_size(ARCHIVE_BLOCK_SIZE);
        // zstd level 3 (backhand's default).
        writer.set_compressor(FilesystemCompressor::new(
            Compressor::Zstd,
            Some(CompressionOptions::Zstd(Zstd {
                compression_level: 3,
            })),
        )?);

        let (spool, spool_path) = create_spool(output)?;

        Ok(Self {
            writer,
            spool,
            spool_path,
            entries: Vec::new(),
            output: output.to_path_buf(),
        })
    }

    pub(crate) fn finalize(mut self) -> anyhow::Result<()> {
        let header = NodeHeader::new(0o755, 1000, 1000, 0);

        let mut parents: Vec<PathBuf> = self
            .entries
            .iter()
            .filter_map(|(path, _, _)| path.parent().map(Path::to_path_buf))
            .collect();
        parents.sort();
        parents.dedup();
        for parent in &parents {
            self.writer.push_dir_all(parent, header)?;
        }

        for (path, offset, len) in &self.entries {
            let reader = SpoolReader {
                spool: Arc::clone(&self.spool),
                offset: *offset,
                remaining: *len,
            };
            self.writer.push_file(reader, path, header)?;
        }

        let output = File::create(&self.output)
            .with_context(|| format!("failed to create archive {}", self.output.display()))?;
        self.writer.write(output)?;
        Ok(())
    }
}

impl AssetSink for SquashfsAssetSink {
    fn write(&mut self, record: AssetRecord) -> anyhow::Result<()> {
        let path = record.path().to_path_buf();
        let mut spool = self.spool.lock().unwrap();
        let offset = spool.stream_position()?;
        let size = record.size();
        std::io::copy(&mut record.into_reader(), &mut *spool)?;
        self.entries.push((path, offset, size));
        Ok(())
    }
}

/// Reads one record from the shared spool file; seeks per read so they can
/// share a single file handle.
struct SpoolReader {
    spool: Arc<Mutex<File>>,
    offset: u64,
    remaining: u64,
}

impl Read for SpoolReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.remaining == 0 || buf.is_empty() {
            return Ok(0);
        }

        let mut spool = self.spool.lock().unwrap();
        spool.seek(SeekFrom::Start(self.offset))?;
        let want = buf.len().min(self.remaining as usize);
        let read = spool.read(&mut buf[..want])?;
        self.offset += read as u64;
        self.remaining -= read as u64;
        Ok(read)
    }
}

impl Drop for SquashfsAssetSink {
    fn drop(&mut self) {
        match std::fs::remove_file(&self.spool_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::warn!(
                    "failed to remove spool file {}: {error}",
                    self.spool_path.display()
                );
            }
        }
    }
}

fn create_spool(output: &Path) -> anyhow::Result<(Arc<Mutex<File>>, PathBuf)> {
    let spool_dir = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(spool_dir)?;

    for _ in 0..100 {
        let name = format!(
            "data.squashfs.spool-{}-{}.tmp",
            std::process::id(),
            SPOOL_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let path = spool_dir.join(name);
        match File::create_new(&path) {
            Ok(file) => return Ok((Arc::new(Mutex::new(file)), path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    anyhow::bail!(
        "could not create a unique spool file in {}",
        spool_dir.display()
    )
}

#[cfg(test)]
mod tests {
    use super::{AssetSink, SquashfsAssetSink};
    use crate::asset_record::AssetRecord;
    use backhand::{FilesystemReader, InnerNode};
    use std::io::{BufReader, Read};
    use std::path::Path;
    use std::sync::atomic::Ordering;

    fn fullpath_matches(fullpath: &Path, expected: &str) -> bool {
        let is_normal =
            |component: &std::path::Component| matches!(component, std::path::Component::Normal(_));
        fullpath
            .components()
            .filter(is_normal)
            .eq(Path::new(expected).components().filter(is_normal))
    }

    #[test]
    fn finalize_writes_readable_squashfs_archive() {
        let dir = std::env::temp_dir().join(format!(
            "talgonite-sink-test-{}-{}",
            std::process::id(),
            super::SPOOL_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("test.squashfs");

        let mut sink = SquashfsAssetSink::new(&archive_path).unwrap();
        sink.write(AssetRecord::bytes(
            Path::new("Legend/file.bin"),
            b"abc".to_vec(),
        ))
        .unwrap();
        let large = b"hello world".repeat(1000);
        sink.write(AssetRecord::bytes(Path::new("ia/sotp.dat"), large.clone()))
            .unwrap();
        sink.write(AssetRecord::bytes(Path::new("VERSION"), b"741_5".to_vec()))
            .unwrap();
        sink.write(AssetRecord::bytes(Path::new("Music/empty.bin"), vec![]))
            .unwrap();
        sink.write(AssetRecord::bytes(
            Path::new("a/b/c/d.bin"),
            b"deep".to_vec(),
        ))
        .unwrap();
        sink.finalize().unwrap();

        let fs = FilesystemReader::from_reader(BufReader::new(
            std::fs::File::open(&archive_path).unwrap(),
        ))
        .unwrap();

        let read_file = |path: &str| -> Vec<u8> {
            for node in fs.files() {
                if fullpath_matches(&node.fullpath, path) {
                    if let InnerNode::File(file) = &node.inner {
                        let mut reader = fs.file(file).reader();
                        let mut buf = vec![];
                        reader.read_to_end(&mut buf).unwrap();
                        return buf;
                    }
                }
            }
            panic!("missing file {}", path);
        };

        assert_eq!(read_file("/Legend/file.bin"), b"abc");
        assert_eq!(read_file("/ia/sotp.dat"), large);
        assert_eq!(read_file("/VERSION"), b"741_5");
        assert_eq!(read_file("/Music/empty.bin"), b"");
        assert_eq!(read_file("/a/b/c/d.bin"), b"deep");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn finalize_handles_many_entries_without_exhausting_file_descriptors() {
        let dir = std::env::temp_dir().join(format!(
            "talgonite-sink-test-{}-{}",
            std::process::id(),
            super::SPOOL_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let archive_path = dir.join("many.squashfs");

        let mut sink = SquashfsAssetSink::new(&archive_path).unwrap();
        for i in 0..2500 {
            sink.write(AssetRecord::bytes(
                Path::new(&format!("dir{}/file{i}.bin", i % 16)),
                format!("content {i}").into_bytes(),
            ))
            .unwrap();
        }
        sink.finalize().unwrap();

        let fs = FilesystemReader::from_reader(BufReader::new(
            std::fs::File::open(&archive_path).unwrap(),
        ))
        .unwrap();
        assert_eq!(fs.files().count(), 2500 + 16 + 1); // files + dirs + root

        let mut found = None;
        for node in fs.files() {
            if fullpath_matches(&node.fullpath, "/dir15/file1999.bin") {
                if let InnerNode::File(file) = &node.inner {
                    let mut reader = fs.file(file).reader();
                    let mut buf = vec![];
                    reader.read_to_end(&mut buf).unwrap();
                    found = Some(buf);
                }
            }
        }
        assert_eq!(found, Some(b"content 1999".to_vec()));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
