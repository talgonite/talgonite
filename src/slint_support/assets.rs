use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use tracing::debug;

use crate::game_files::GameFiles;
use crate::metafile_store::MetafileStore;

pub struct SlintAssetLoader {
    item_palette_table: rangemap::RangeMap<u16, u16>,
}

impl SlintAssetLoader {
    pub fn new(game_files: &GameFiles) -> Self {
        let table_data = game_files
            .get_file("Legend/item.tbl.bin")
            .expect("item palette table missing");
        let (item_palette_table, _): (rangemap::RangeMap<u16, u16>, usize) =
            bincode::serde::decode_from_slice(&table_data, bincode::config::standard()).unwrap();

        Self { item_palette_table }
    }

    pub fn load_item_icon(&self, game_files: &GameFiles, sprite_id: u16) -> Result<Image, String> {
        const ITEMS_PER_FILE: u16 = 266;
        let zero_based = sprite_id.saturating_sub(1) as u16;
        let file_index = zero_based / ITEMS_PER_FILE + 1;
        let index_in_file = (zero_based % ITEMS_PER_FILE) as usize;
        let epf_path = format!("Legend/item{:03}.epf.bin", file_index);

        SlintAssetLoader::decode_epf_to_slint(
            game_files,
            &epf_path,
            index_in_file,
            "Legend/item.ktx2",
            self.item_palette_table
                .get(&sprite_id)
                .copied()
                .unwrap_or_default() as usize,
        )
    }

    pub fn load_npc_portrait(
        &self,
        game_files: &GameFiles,
        metafile_store: &MetafileStore,
        sprite_id: u16,
        npc_name: Option<&str>,
    ) -> Result<Image, String> {
        if let Some(npc_name) = npc_name {
            if let Some(meta) = metafile_store.get_metafile_data("NPCIllust") {
                if let Some(entry) = meta.entries.iter().find(|e| e.name == npc_name) {
                    if let Some(spf_name) = entry.fields.first() {
                        let filename = spf_name.to_lowercase().replace(".spf", ".0.ktx2");
                        let full_path = format!("npc/npcbase/{}", filename);

                        if let Some(bytes) = game_files.get_file(&full_path) {
                            let (w, h, data) = rendering::texture::Texture::load_ktx2(&bytes)
                                .map_err(|e| e.to_string())?;
                            let mut pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::new(w, h);
                            pixel_buffer
                                .make_mut_slice()
                                .copy_from_slice(bytemuck::cast_slice(&data));
                            debug!(
                                "NPC portrait {} (name: {}) loaded from metafile path {}",
                                sprite_id, npc_name, full_path
                            );
                            return Ok(Image::from_rgba8(pixel_buffer));
                        }
                    }
                }
            }

            debug!(
                "NPC {} not found in portrait map, falling back to MPF",
                npc_name
            );
        }

        let mpf_path = format!("hades/mns{:03}.mpf.bin", sprite_id);
        let mpf_bytes = game_files
            .get_file(&mpf_path)
            .ok_or_else(|| format!("MPF not found: {}", mpf_path))?;
        let (mpf_file, _): (formats::mpf::MpfFile, _) =
            bincode::decode_from_slice(&mpf_bytes, bincode::config::standard())
                .map_err(|e| e.to_string())?;

        let frame_index = if let Some(anim) = mpf_file
            .animations
            .iter()
            .find(|a| a.animation_type == formats::mpf::MpfAnimationType::Standing)
        {
            anim.frame_index_for_direction(formats::epf::AnimationDirection::Towards) as usize
        } else {
            0
        };

        if frame_index >= mpf_file.frames.len() {
            return Err(format!("Frame index {} out of range", frame_index));
        }

        let frame = &mpf_file.frames[frame_index];
        let w = (frame.right - frame.left).max(1) as u32;
        let h = (frame.bottom - frame.top).max(1) as u32;

        let palette_path = "hades/mns.ktx2";
        let palette_bytes = game_files
            .get_file(palette_path)
            .ok_or_else(|| format!("Palette not found: {}", palette_path))?;
        let (_, _, pal_data) = rendering::texture::Texture::load_ktx2(&palette_bytes)
            .map_err(|e| format!("palette load: {e}"))?;

        let palette_size = 4 * 256;
        let palette_index = mpf_file.palette_number as usize;
        let total_palettes = pal_data.len() / palette_size;

        if palette_index >= total_palettes {
            return Err(format!(
                "palette index {palette_index} out of range (total {total_palettes})"
            ));
        }
        let pal_offset = palette_size * palette_index;
        let palette_rgba = &pal_data[pal_offset..pal_offset + palette_size];

        let mut pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::new(w, h);
        let pixels = pixel_buffer.make_mut_slice();

        let frame_indices = &frame.data[..(w * h) as usize];

        for (i, &idx) in frame_indices.iter().enumerate() {
            if idx == 0 {
                pixels[i] = Rgba8Pixel {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                };
            } else {
                let pal_idx = idx as usize * 4;
                pixels[i] = Rgba8Pixel {
                    r: palette_rgba[pal_idx],
                    g: palette_rgba[pal_idx + 1],
                    b: palette_rgba[pal_idx + 2],
                    a: palette_rgba[pal_idx + 3],
                };
            }
        }

        Ok(Image::from_rgba8(pixel_buffer))
    }

    pub fn load_skill_icon(&self, game_files: &GameFiles, sprite_id: u16) -> Result<Image, String> {
        SlintAssetLoader::decode_epf_to_slint(
            game_files,
            &"setoa/skill001.epf.bin",
            sprite_id as usize,
            "setoa/gui.ktx2",
            6,
        )
    }

    pub fn load_spell_icon(&self, game_files: &GameFiles, sprite_id: u16) -> Result<Image, String> {
        SlintAssetLoader::decode_epf_to_slint(
            game_files,
            &"setoa/spell001.epf.bin",
            sprite_id as usize,
            "setoa/gui.ktx2",
            6,
        )
    }

    pub fn load_world_map_image(
        &self,
        game_files: &GameFiles,
        field_name: &str,
    ) -> Result<Image, String> {
        let epf_path = format!("setoa/{}.epf.bin", field_name);
        let pal_path = format!("setoa/{}.pal", field_name);

        let pal_bytes = game_files
            .get_file(&pal_path)
            .ok_or_else(|| format!("Palette file not found: {}", pal_path))?;
        if pal_bytes.len() < 768 {
            return Err(format!("Palette file too small: {}", pal_path));
        }

        let mut palette_rgba = vec![0u8; 256 * 4];
        for i in 0..256 {
            palette_rgba[i * 4] = pal_bytes[i * 3];
            palette_rgba[i * 4 + 1] = pal_bytes[i * 3 + 1];
            palette_rgba[i * 4 + 2] = pal_bytes[i * 3 + 2];
            palette_rgba[i * 4 + 3] = 255;
        }

        SlintAssetLoader::decode_epf_to_slint_with_palette(game_files, &epf_path, 0, &palette_rgba)
    }

    fn decode_epf_to_slint(
        game_files: &GameFiles,
        epf_path: &str,
        frame_index: usize,
        palette_path: &str,
        palette_index: usize,
    ) -> Result<Image, String> {
        let palette_rgba = {
            let palette_bytes = game_files
                .get_file(palette_path)
                .ok_or_else(|| format!("Palette file not found: {}", palette_path))?;
            if palette_bytes.is_empty() {
                return Err(format!("Palette file not found: {}", palette_path));
            }
            let (_, _, pal_data) = rendering::texture::Texture::load_ktx2(&palette_bytes)
                .map_err(|e| format!("palette load: {e}"))?;
            let palette_size = 4 * 256;

            let total_palettes = pal_data.len() / palette_size;

            if palette_index >= total_palettes {
                return Err(format!(
                    "palette index {palette_index} out of range (total {total_palettes})"
                ));
            }
            let pal_offset = palette_size * palette_index;
            let slice = &pal_data[pal_offset..pal_offset + palette_size];
            slice.to_vec()
        };

        SlintAssetLoader::decode_epf_to_slint_with_palette(
            game_files,
            epf_path,
            frame_index,
            &palette_rgba,
        )
    }

    fn decode_epf_to_slint_with_palette(
        game_files: &GameFiles,
        epf_path: &str,
        frame_index: usize,
        palette_rgba: &[u8],
    ) -> Result<Image, String> {
        let epf_bytes = game_files
            .get_file(epf_path)
            .ok_or_else(|| format!("EPF file not found: {}", epf_path))?;
        if epf_bytes.is_empty() {
            return Err(format!("EPF file not found: {}", epf_path));
        }

        let (epf_image, _): (formats::epf::EpfImage, _) =
            bincode::decode_from_slice(&epf_bytes, bincode::config::standard())
                .map_err(|e| format!("decode epf: {e}"))?;

        if frame_index >= epf_image.frames.len() {
            return Err("frame index out of range".into());
        }

        let frame = &epf_image.frames[frame_index];
        let w = frame.right.saturating_sub(frame.left).max(1) as u32;
        let h = frame.bottom.saturating_sub(frame.top).max(1) as u32;

        if frame.data.len() < (w * h) as usize {
            return Err("frame data truncated".into());
        }

        let mut pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::new(w, h);
        let pixels = pixel_buffer.make_mut_slice();

        let frame_indices = &frame.data[..(w * h) as usize];

        for (i, &idx) in frame_indices.iter().enumerate() {
            if idx == 0 {
                pixels[i] = Rgba8Pixel {
                    r: 0,
                    g: 0,
                    b: 0,
                    a: 0,
                };
            } else {
                let pal_idx = idx as usize * 4;
                pixels[i] = Rgba8Pixel {
                    r: palette_rgba[pal_idx],
                    g: palette_rgba[pal_idx + 1],
                    b: palette_rgba[pal_idx + 2],
                    a: palette_rgba[pal_idx + 3],
                };
            }
        }

        Ok(Image::from_rgba8(pixel_buffer))
    }
}
