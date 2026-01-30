use bevy::prelude::*;
use std::fs;
use std::path::PathBuf;
use tracing::{error, info};

#[derive(Resource)]
pub struct MapStore;

impl MapStore {
    pub fn new() -> Self {
        Self
    }

    fn map_path(&self, server_id: u32, id: u16) -> PathBuf {
        crate::server_maps_dir(server_id).join(format!("lod{:03}.map", id))
    }

    pub fn get_map(&self, server_id: u32, id: u16) -> Option<Vec<u8>> {
        let path = self.map_path(server_id, id);
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

    pub fn save_map(&self, server_id: u32, id: u16, data: &[u8]) {
        let path = self.map_path(server_id, id);
        if let Err(e) = fs::write(&path, data) {
            error!("Failed to save map file {:?}: {}", path, e);
        } else {
            info!("Saved map {} to disk", id);
        }
    }
}
