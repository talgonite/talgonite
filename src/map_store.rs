use crate::storage_dir;
use bevy::prelude::*;
use std::fs;
use std::path::PathBuf;
use tracing::{error, info};

#[derive(Resource)]
pub struct MapStore {
    base_path: PathBuf,
}

impl MapStore {
    pub fn new() -> Self {
        let mut base_path = storage_dir();
        base_path.push("maps");
        if let Err(e) = fs::create_dir_all(&base_path) {
            error!("Failed to create maps directory: {}", e);
        }
        Self { base_path }
    }

    fn map_path(&self, id: u16) -> PathBuf {
        self.base_path.join(format!("lod{:03}.map", id))
    }

    pub fn get_map(&self, id: u16) -> Option<Vec<u8>> {
        let path = self.map_path(id);
        if !path.exists() {
            return None;
        }

        match fs::read(&path) {
            Ok(data) => Some(data),
            Err(e) => {
                error!("Failed to read map file {:?}: {}", path, e);
                None
            }
        }
    }

    pub fn save_map(&self, id: u16, data: &[u8]) {
        let path = self.map_path(id);
        if let Err(e) = fs::write(&path, data) {
            error!("Failed to save map file {:?}: {}", path, e);
        } else {
            info!("Saved map {} to disk", id);
        }
    }
}
