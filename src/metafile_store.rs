use bevy::prelude::*;
use flate2::read::ZlibDecoder;
use formats::meta_file::MetaFile;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use tracing::{error, info};

fn crc32(data: &[u8]) -> u32 {
    let mut crc = flate2::Crc::new();
    crc.update(data);
    crc.sum()
}

#[derive(Resource)]
pub struct MetafileStore {
    server_id: Option<u32>,
    metafiles: HashMap<String, MetaFile>,
}

impl MetafileStore {
    pub fn new() -> Self {
        Self {
            server_id: None,
            metafiles: HashMap::new(),
        }
    }

    pub fn set_server(&mut self, server_id: u32) {
        if self.server_id == Some(server_id) {
            return;
        }
        self.server_id = Some(server_id);
        self.reload_all();
    }

    fn base_path(&self) -> Option<PathBuf> {
        self.server_id.map(crate::server_metafile_dir)
    }

    pub fn reload_all(&mut self) {
        let Some(base_path) = self.base_path() else {
            return;
        };

        if let Ok(entries) = fs::read_dir(&base_path) {
            self.metafiles.clear();
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_file() {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        if let Ok(data) = fs::read(entry.path()) {
                            match MetaFile::from_bytes(&data) {
                                Ok(metafile) => {
                                    self.metafiles.insert(name, metafile);
                                }
                                Err(e) => {
                                    error!("Failed to parse metafile {}: {}", name, e);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn get_metafile_data(&self, name: &str) -> Option<&MetaFile> {
        self.metafiles.get(name)
    }

    fn metafile_path(&self, name: &str) -> Option<PathBuf> {
        self.base_path().map(|p| p.join(name))
    }

    /// Returns the metafile data and its checksum (as u32 for comparison with server)
    pub fn get_metafile(&self, name: &str) -> Option<(Vec<u8>, u32)> {
        let path = self.metafile_path(name)?;
        if !path.exists() {
            return None;
        }

        match fs::read(&path) {
            Ok(data) => {
                let checksum = crc32(&data);
                Some((data, checksum))
            }
            Err(e) => {
                error!("Failed to read metafile {:?}: {}", path, e);
                None
            }
        }
    }

    /// Get just the checksum of a metafile without loading all the data
    pub fn get_checksum(&self, name: &str) -> Option<u32> {
        let path = self.metafile_path(name)?;
        if !path.exists() {
            return None;
        }

        match fs::read(&path) {
            Ok(data) => Some(crc32(&data)),
            Err(e) => {
                error!("Failed to read metafile {:?} for checksum: {}", path, e);
                None
            }
        }
    }

    /// Save a metafile to disk. Data from server is zlib-compressed.
    /// Decompresses, verifies checksum, and saves decompressed data.
    pub fn save_metafile(
        &mut self,
        name: &str,
        compressed_data: &[u8],
        expected_checksum: u32,
    ) -> bool {
        let Some(path) = self.metafile_path(name) else {
            return false;
        };

        // Decompress the data
        let mut decoder = ZlibDecoder::new(compressed_data);
        let mut decompressed = Vec::new();
        if let Err(e) = decoder.read_to_end(&mut decompressed) {
            error!("Failed to decompress metafile {}: {}", name, e);
            return false;
        }

        // Verify checksum of decompressed data
        let actual_checksum = crc32(&decompressed);
        if actual_checksum != expected_checksum {
            error!(
                "Metafile {} checksum mismatch (expected {}, got {})",
                name, expected_checksum, actual_checksum
            );
            return false;
        }

        // Save decompressed data to disk
        if let Err(e) = fs::write(&path, &decompressed) {
            error!("Failed to save metafile {:?}: {}", path, e);
            return false;
        }

        // Update cache
        match MetaFile::from_bytes(&decompressed) {
            Ok(metafile) => {
                self.metafiles.insert(name.to_string(), metafile);
            }
            Err(e) => {
                error!("Failed to parse metafile {} after saving: {}", name, e);
            }
        }

        info!(
            "Saved metafile {} to disk ({} bytes)",
            name,
            decompressed.len()
        );
        true
    }
}
