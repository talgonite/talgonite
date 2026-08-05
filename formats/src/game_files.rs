const MAX_ARCHIVE_SHARDS: usize = 4;

#[derive(Clone)]
pub struct SquashfsArchive {
    /// One reader (and file handle) per shard, so parallel reads stay concurrent.
    archives: std::sync::Arc<Vec<backhand::FilesystemReader<'static>>>,
    index: std::collections::HashMap<String, usize>,
}

impl SquashfsArchive {
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        use std::io::BufReader;

        let path = path.as_ref();
        let shard_count = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .clamp(1, MAX_ARCHIVE_SHARDS);

        let mut archives = Vec::with_capacity(shard_count);
        for _ in 0..shard_count {
            let archive: backhand::FilesystemReader<'static> =
                backhand::FilesystemReader::from_reader(BufReader::new(std::fs::File::open(
                    path,
                )?))?;
            archives.push(archive);
        }

        // All shards parse the same filesystem, so node indices match.
        let index = archives[0]
            .files()
            .enumerate()
            .filter_map(|(node_index, node)| match &node.inner {
                backhand::InnerNode::File(_) => {
                    Some((archive_path_key(&node.fullpath), node_index))
                }
                _ => None,
            })
            .collect();

        Ok(Self {
            archives: std::sync::Arc::new(archives),
            index,
        })
    }

    #[tracing::instrument(level = "info", skip(self), fields(path = %path))]
    pub fn get_file(&self, path: &str) -> Result<Vec<u8>, SquashfsError> {
        use std::io::Read;

        let node_index = self
            .index
            .get(&archive_path_key(std::path::Path::new(path)))
            .ok_or_else(|| SquashfsError::FileNotFound(path.to_string()))?;
        let node = &self.archives[0].root.nodes[*node_index];
        let backhand::InnerNode::File(file) = &node.inner else {
            return Err(SquashfsError::FileNotFound(path.to_string()));
        };

        let mut reader = self.archives[0].file(file).reader();
        let mut buf = vec![];
        reader.read_to_end(&mut buf)?;
        Ok(buf)
    }

    #[tracing::instrument(
        level = "info",
        skip(self, paths),
        fields(file_count = paths.len())
    )]
    pub fn get_files_parallel<S>(&self, paths: &[S]) -> Vec<Result<Vec<u8>, SquashfsError>>
    where
        S: AsRef<str> + Sync,
    {
        use std::io::Read;

        let shard_count = self.archives.len();

        if paths.len() <= 1 {
            let results: Vec<_> = paths
                .iter()
                .map(|path| self.get_file(path.as_ref()))
                .collect();

            return results;
        }

        let mut results = paths
            .iter()
            .map(|path| Err(SquashfsError::FileNotFound(path.as_ref().to_string())))
            .collect::<Vec<_>>();

        // Shard by stable node index: one worker per shard, no shared-handle contention.
        let mut shard_jobs: Vec<Vec<(usize, usize)>> = vec![Vec::new(); shard_count];
        for (path_index, path) in paths.iter().enumerate() {
            if let Some(&node_index) = self
                .index
                .get(&archive_path_key(std::path::Path::new(path.as_ref())))
            {
                shard_jobs[node_index % shard_count].push((path_index, node_index));
            }
        }

        std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for (shard, jobs) in shard_jobs.iter().enumerate() {
                if jobs.is_empty() {
                    continue;
                }
                let archive = &self.archives[shard];
                handles.push(scope.spawn(move || {
                    jobs.iter()
                        .map(|&(path_index, node_index)| {
                            let node = &archive.root.nodes[node_index];
                            let result = match &node.inner {
                                backhand::InnerNode::File(file) => {
                                    let mut reader = archive.file(file).reader();
                                    let mut buf = vec![];
                                    reader
                                        .read_to_end(&mut buf)
                                        .map(|_| buf)
                                        .map_err(Into::into)
                                }
                                _ => Err(SquashfsError::FileNotFound(
                                    paths[path_index].as_ref().to_string(),
                                )),
                            };
                            (path_index, result)
                        })
                        .collect::<Vec<_>>()
                }));
            }

            for handle in handles {
                for (path_index, result) in handle.join().expect("archive read worker panicked") {
                    results[path_index] = result;
                }
            }
        });

        results
    }

    pub fn get_file_or_panic(&self, path: &str) -> Vec<u8> {
        match self.get_file(path) {
            Ok(data) => data,
            Err(e) => panic!("Failed to get file '{}': {}", path, e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use backhand::{
        FilesystemCompressor, FilesystemWriter, NodeHeader,
        compression::{CompressionOptions, Compressor, Zstd},
        kind,
        kind::Kind,
    };
    use std::path::Path;

    fn write_test_archive(path: &Path) {
        let mut fs = FilesystemWriter::default();
        fs.set_kind(Kind::from_const(kind::LE_V4_0).unwrap());
        fs.set_block_size(8 * 1024);
        fs.set_compressor(
            FilesystemCompressor::new(
                Compressor::Zstd,
                Some(CompressionOptions::Zstd(Zstd {
                    compression_level: 3,
                })),
            )
            .unwrap(),
        );
        let header = NodeHeader::new(0o755, 1000, 1000, 0);
        fs.push_dir_all("ia", header).unwrap();
        fs.push_dir_all("seo", header).unwrap();
        fs.push_file(std::io::Cursor::new(b"alpha".to_vec()), "ia/a.dat", header)
            .unwrap();
        fs.push_file(std::io::Cursor::new(vec![7; 4096]), "ia/b.dat", header)
            .unwrap();
        fs.push_file(std::io::Cursor::new(b"gamma".to_vec()), "seo/g.dat", header)
            .unwrap();
        let mut out = std::fs::File::create(path).unwrap();
        fs.write(&mut out).unwrap();
    }

    #[test]
    fn parallel_reads_match_single_reads() {
        let dir =
            std::env::temp_dir().join(format!("talgonite-formats-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.squashfs");
        write_test_archive(&path);

        let archive = SquashfsArchive::new(&path).unwrap();
        let paths = ["ia/a.dat", "ia/b.dat", "seo/g.dat", "missing.dat"];
        let results = archive.get_files_parallel(&paths);

        assert_eq!(results[0].as_deref().unwrap(), b"alpha");
        assert_eq!(results[1].as_deref().unwrap(), vec![7; 4096]);
        assert_eq!(results[2].as_deref().unwrap(), b"gamma");
        assert!(matches!(results[3], Err(SquashfsError::FileNotFound(_))));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}

fn archive_path_key(path: &std::path::Path) -> String {
    // Components-based normalization keeps keys identical on Windows, where
    // backhand fullpaths use backslash separators.
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[derive(Debug, Clone)]
pub enum SquashfsError {
    FileNotFound(String),
    IoError(String),
    ArchiveError(String),
}

impl std::fmt::Display for SquashfsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SquashfsError::FileNotFound(path) => write!(f, "File not found: {}", path),
            SquashfsError::IoError(err) => write!(f, "IO error: {}", err),
            SquashfsError::ArchiveError(msg) => write!(f, "Archive error: {}", msg),
        }
    }
}

impl std::error::Error for SquashfsError {}

impl From<std::io::Error> for SquashfsError {
    fn from(err: std::io::Error) -> Self {
        SquashfsError::IoError(err.to_string())
    }
}

// Simple wrapper around the SquashFS archive, shared by desktop and Android.
pub struct GameFiles {
    archive: SquashfsArchive,
}

impl GameFiles {
    pub fn new(archive_path: &str) -> Self {
        let archive = SquashfsArchive::new(archive_path)
            .map_err(|e| format!("Failed to open game archive at '{}': {:?}", archive_path, e))
            .expect("Failed to open game archive");
        Self { archive }
    }

    pub fn from_archive(archive: SquashfsArchive) -> Self {
        Self { archive }
    }

    pub fn archive(&self) -> &SquashfsArchive {
        &self.archive
    }

    pub fn get_file_or_panic(&self, path: &str) -> Vec<u8> {
        self.archive.get_file_or_panic(path)
    }

    pub fn get_file(&self, path: &str) -> Option<Vec<u8>> {
        self.archive.get_file(path).ok()
    }

    pub fn get_files_parallel<S>(&self, paths: &[S]) -> Vec<Result<Vec<u8>, SquashfsError>>
    where
        S: AsRef<str> + Sync,
    {
        self.archive.get_files_parallel(paths)
    }
}
