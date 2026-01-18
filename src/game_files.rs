use bevy::prelude::*;
use formats::game_files::GameFiles as FormatGameFiles;

use crate::storage_dir;

// Bevy-specific wrapper around the shared GameFiles
#[derive(Resource)]
pub struct GameFiles {
    inner: FormatGameFiles,
}

impl GameFiles {
    pub fn new() -> Self {
        let mut path = storage_dir();
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

    pub fn inner(&self) -> &FormatGameFiles {
        &self.inner
    }
}
