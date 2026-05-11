use jubako::creator::InputReader;
use std::io::Cursor;
use std::path::{Path, PathBuf};

pub(crate) struct AssetRecord {
    path: PathBuf,
    content: AssetContent,
}

impl AssetRecord {
    pub(crate) fn bytes(path: impl Into<PathBuf>, bytes: Vec<u8>) -> Self {
        Self {
            path: path.into(),
            content: AssetContent::Bytes(bytes),
        }
    }

    pub(crate) fn chunks(path: impl Into<PathBuf>, chunks: Vec<Vec<u8>>) -> Self {
        Self {
            path: path.into(),
            content: AssetContent::Chunks(chunks),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn size(&self) -> u64 {
        self.content.size()
    }

    pub(crate) fn into_input_reader(self) -> Box<dyn InputReader> {
        self.content.into_input_reader()
    }
}

enum AssetContent {
    Bytes(Vec<u8>),
    Chunks(Vec<Vec<u8>>),
}

impl AssetContent {
    fn size(&self) -> u64 {
        match self {
            Self::Bytes(bytes) => bytes.len() as u64,
            Self::Chunks(chunks) => chunks.iter().map(|chunk| chunk.len() as u64).sum(),
        }
    }

    fn into_input_reader(self) -> Box<dyn InputReader> {
        match self {
            Self::Bytes(bytes) => Box::new(Cursor::new(bytes)),
            Self::Chunks(chunks) => {
                let total_size = chunks.iter().map(|chunk| chunk.len()).sum();
                let mut bytes = Vec::with_capacity(total_size);
                for chunk in chunks {
                    bytes.extend_from_slice(&chunk);
                }
                Box::new(Cursor::new(bytes))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AssetRecord;
    use std::io::Read;
    use std::path::Path;

    #[test]
    fn chunked_asset_record_streams_all_segments_in_order() {
        let mut reader = AssetRecord::chunks(
            Path::new("Legend/test.bin"),
            vec![b"ab".to_vec(), b"cd".to_vec(), b"ef".to_vec()],
        )
        .into_input_reader();
        let mut buf = vec![];
        reader.read_to_end(&mut buf).unwrap();

        assert_eq!(buf, b"abcdef");
    }
}