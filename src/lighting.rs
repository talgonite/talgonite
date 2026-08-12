//! `Light` metafile parsing and baked light-texture loading.

use formats::meta_file::MetaFile;
use std::collections::HashMap;
use tracing::warn;

use rendering::scene::darkness::{HeaData, LightMask};

#[derive(Debug, Clone, Copy)]
pub struct LightProperty {
    pub alpha: u8,
    pub color: [u8; 3],
}

#[derive(Debug, Default, Clone)]
pub struct LightMetadata {
    properties: HashMap<String, LightProperty>,
    map_light_types: HashMap<u16, String>,
}

impl LightMetadata {
    pub fn from_metafile(meta: &MetaFile) -> Self {
        let mut properties = HashMap::new();
        let mut map_light_types = HashMap::new();

        for entry in &meta.entries {
            let key = entry.name.to_lowercase();

            if key.contains('_') {
                if entry.fields.len() < 6 {
                    continue;
                }

                let Some(alpha) = entry.fields[2].parse::<u8>().ok() else {
                    continue;
                };
                let Some(r) = entry.fields[3].parse::<u8>().ok() else {
                    continue;
                };
                let Some(g) = entry.fields[4].parse::<u8>().ok() else {
                    continue;
                };
                let Some(b) = entry.fields[5].parse::<u8>().ok() else {
                    continue;
                };

                properties.insert(
                    key,
                    LightProperty {
                        alpha,
                        color: [r, g, b],
                    },
                );
            } else if let Ok(map_id) = key.parse::<u16>() {
                if let Some(light_type) = entry.fields.first() {
                    map_light_types.insert(map_id, light_type.to_lowercase());
                }
            }
        }

        Self {
            properties,
            map_light_types,
        }
    }

    /// True when the map has an explicit light type entry.
    pub fn has_entry(&self, map_id: u16) -> bool {
        self.map_light_types.contains_key(&map_id)
    }

    /// Resolves a light level into `(opacity, rgb)`, or `None` without a
    /// matching entry.
    pub fn resolve(&self, map_id: u16, level: u8) -> Option<(f32, [u8; 3])> {
        if !self.has_entry(map_id) {
            return None;
        }

        let light_type = self
            .map_light_types
            .get(&map_id)
            .map(String::as_str)
            .unwrap_or("default");
        let Some(property) = self
            .properties
            .get(&format!("{}_{:X}", light_type, level).to_lowercase())
        else {
            return None;
        };

        let opacity = if property.alpha >= 32 {
            0.0
        } else {
            (32 - property.alpha) as f32 / 32.0
        };
        Some((opacity, property.color))
    }
}

/// Loads a baked lantern mask (`mask101` = small, `mask102` = large).
pub fn load_light_mask(
    archive: &formats::game_files::SquashfsArchive,
    name: &str,
) -> Option<LightMask> {
    let bytes = archive
        .get_file(&format!("Legend/{name}.light.ktx2"))
        .map_err(|error| warn!(name, %error, "Failed to load lantern mask texture"))
        .ok()?;
    let (width, height, pixels) = rendering::texture::Texture::load_ktx2(&bytes)
        .map_err(|error| warn!(name, %error, "Failed to decode lantern mask texture"))
        .ok()?;

    Some(LightMask {
        width,
        height,
        pixels,
    })
}

/// Loads the map's baked HEA light map, if any.
pub fn load_hea(archive: &formats::game_files::SquashfsArchive, map_id: u16) -> Option<HeaData> {
    let bytes = archive
        .get_file(&format!("seo/{map_id:06}.light.ktx2"))
        .map_err(|error| warn!(map_id, %error, "Map has no baked HEA light map"))
        .ok()?;
    let (width, height, pixels) = rendering::texture::Texture::load_ktx2(&bytes)
        .map_err(|error| warn!(map_id, %error, "Failed to decode HEA light map"))
        .ok()?;

    Some(HeaData {
        width,
        height,
        screen_width: 640.0,
        screen_height: 480.0,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use formats::meta_file::{MetaFile, MetaFileEntry};

    fn meta(entries: &[(&str, &[&str])]) -> MetaFile {
        MetaFile {
            entries: entries
                .iter()
                .map(|(name, fields)| MetaFileEntry {
                    name: name.to_string(),
                    fields: fields.iter().map(|s| s.to_string()).collect(),
                })
                .collect(),
        }
    }

    #[test]
    fn parses_light_properties_and_map_types() {
        let meta = meta(&[
            ("Default_0", &["0", "1", "18", "6", "11", "60"]),
            ("Default_a", &["20", "21", "23", "100", "10", "100"]),
            ("Default_b", &["22", "24", "20", "27", "1", "59"]),
            ("Default_4", &["8", "9", "32", "0", "0", "255"]),
            ("500", &["Default"]),
            ("501", &["Default"]),
        ]);

        let light = LightMetadata::from_metafile(&meta);
        assert!(light.has_entry(500));
        assert!(!light.has_entry(502));

        let (alpha, color) = light.resolve(500, 0).unwrap();
        assert!((alpha - 14.0 / 32.0).abs() < 1e-6);
        assert_eq!(color, [6, 11, 60]);

        // Level 4 is alpha 32 -> fully bright -> zero opacity.
        assert_eq!(light.resolve(500, 4).unwrap().0, 0.0);
        // Hex letter levels (10/11) resolve against the lowercase metafile keys.
        let (alpha, color) = light.resolve(500, 10).unwrap();
        assert!((alpha - 9.0 / 32.0).abs() < 1e-6);
        assert_eq!(color, [100, 10, 100]);
        let (alpha, color) = light.resolve(500, 11).unwrap();
        assert!((alpha - 12.0 / 32.0).abs() < 1e-6);
        assert_eq!(color, [27, 1, 59]);
        // Maps without an entry are untouched by light level packets.
        assert!(light.resolve(502, 0).is_none());
    }
}
