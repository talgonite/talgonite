use bevy::prelude::*;
use formats::game_files::GameFiles as FormatGameFiles;

// Bevy-specific wrapper around the shared GameFiles
#[derive(Resource)]
pub struct GameFiles {
    inner: FormatGameFiles,
}

impl GameFiles {
    pub fn from_root(root: &std::path::Path) -> Self {
        let mut path = root.to_path_buf();
        path.push("data.arx");

        let inner = FormatGameFiles::new(path.to_str().expect("invalid path"));
        Self { inner }
    }

    pub fn from_archive(archive: formats::game_files::ArxArchive) -> Self {
        let inner = FormatGameFiles::from_archive(archive);
        Self { inner }
    }

    pub fn get_file(&self, path: &str) -> Option<Vec<u8>> {
        self.inner.get_file(path)
    }

    /// Read many files at once, using a worker pool to parallelize archive
    /// lookups and decompression (mirrors the player-sprite loading path).
    pub fn get_files_parallel<S>(&self, paths: &[S]) -> Vec<Result<Vec<u8>, formats::game_files::ArxError>>
    where
        S: AsRef<str> + Sync,
    {
        self.inner.get_files_parallel(paths)
    }

    pub fn inner(&self) -> &FormatGameFiles {
        &self.inner
    }
}
