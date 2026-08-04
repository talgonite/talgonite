use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use formats::epf::EpfImage;
use formats::game_files::ArxError;
use formats::util::parallel_indexed;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use tracing::debug;

use crate::game_files::GameFiles;
use crate::metafile_store::MetafileStore;

/// Which icon sheet a sprite id refers to. Icons with the same kind share the
/// same EPF file layout and palette, so sprite ids are only meaningful within
/// a kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconKind {
    Item,
    Skill,
    Spell,
}

/// Cache of decoded icon pixels: `(kind, sprite_id)` -> decoded RGBA buffer.
/// `None` entries remember failed loads.
type IconCache = HashMap<(IconKind, u16), Option<SharedPixelBuffer<Rgba8Pixel>>>;

pub struct SlintAssetLoader {
    item_palette_table: rangemap::RangeMap<u16, u16>,
    /// Decoded icon pixels keyed by `(kind, sprite_id)`. We cache raw pixel
    /// buffers rather than `slint::Image` because the latter is not `Send`
    /// (it can hold backend texture handles) and this resource is shared
    /// across threads. `None` entries remember failed loads so we do not
    /// retry them on every UI refresh.
    icon_cache: RwLock<IconCache>,
}

/// One icon in a batch, with the exact file + frame + palette it needs.
struct IconLoadPlan {
    epf_path: String,
    palette_path: String,
    palette_index: usize,
    frame_index: usize,
}

impl SlintAssetLoader {
    pub fn new(game_files: &GameFiles) -> Self {
        let table_data = game_files
            .get_file("Legend/item.tbl.bin")
            .expect("item palette table missing");
        let (item_palette_table, _): (rangemap::RangeMap<u16, u16>, usize) =
            oxicode::serde::decode_from_slice(&table_data, oxicode::config::standard()).unwrap();

        Self {
            item_palette_table,
            icon_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Resolve a batch of icons, loading any uncached sprite ids in parallel
    /// (shared file reads + shared decode work, like player parts). Results are
    /// returned in the same order as `requests`; failed or missing icons yield
    /// `None` and are logged once when first encountered.
    pub fn icons(
        &self,
        game_files: &GameFiles,
        requests: &[(IconKind, u16)],
    ) -> Vec<Option<Image>> {
        if requests.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<Option<Image>> = Vec::with_capacity(requests.len());
        let mut missing: Vec<(IconKind, u16)> = Vec::new();
        let mut missing_indices: Vec<usize> = Vec::new();

        {
            let cache = self.icon_cache.read().expect("icon cache poisoned");
            for (request_index, &request) in requests.iter().enumerate() {
                match cache.get(&request) {
                    Some(icon) => results.push(icon.clone().map(Image::from_rgba8)),
                    None => {
                        results.push(None);
                        missing.push(request);
                        missing_indices.push(request_index);
                    }
                }
            }
        }

        if missing.is_empty() {
            return results;
        }

        // Deduplicate the missing set so each sprite is decoded exactly once.
        let mut unique: Vec<(IconKind, u16)> = Vec::with_capacity(missing.len());
        let mut unique_index: HashMap<(IconKind, u16), usize> =
            HashMap::with_capacity(missing.len());
        for request in &missing {
            if !unique_index.contains_key(request) {
                unique_index.insert(*request, unique.len());
                unique.push(*request);
            }
        }

        let loaded = self.load_icons_batch(game_files, &unique);

        {
            let mut cache = self.icon_cache.write().expect("icon cache poisoned");
            for (request, result) in unique.iter().zip(&loaded) {
                match result {
                    Ok(buffer) => {
                        cache.insert(*request, Some(buffer.clone()));
                    }
                    Err(err) => {
                        tracing::warn!(
                            "Failed to load {:?} icon sprite {}: {}",
                            request.0,
                            request.1,
                            err
                        );
                        cache.insert(*request, None);
                    }
                }
            }
        }

        for (request, request_index) in missing.into_iter().zip(missing_indices) {
            results[request_index] = loaded[unique_index[&request]]
                .as_ref()
                .ok()
                .cloned()
                .map(Image::from_rgba8);
        }

        results
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
            oxicode::decode_from_slice(&mpf_bytes).map_err(|e| e.to_string())?;

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

        let epf_bytes = game_files
            .get_file(&epf_path)
            .ok_or_else(|| format!("EPF file not found: {}", epf_path))?;
        let epf_image = SlintAssetLoader::decode_epf_image(&epf_bytes)?;
        let buffer = SlintAssetLoader::build_frame_buffer(&epf_image, 0, &palette_rgba)?;
        Ok(Image::from_rgba8(buffer))
    }

    /// Load `requests` (assumed deduplicated) in parallel and return a result
    /// per request. File reads go through the archive's parallel reader, then
    /// EPF decode and pixel expansion are spread across worker threads.
    fn load_icons_batch(
        &self,
        game_files: &GameFiles,
        requests: &[(IconKind, u16)],
    ) -> Vec<Result<SharedPixelBuffer<Rgba8Pixel>, String>> {
        let plans: Vec<IconLoadPlan> = requests
            .iter()
            .map(|&(kind, sprite)| match kind {
                IconKind::Item => {
                    const ITEMS_PER_FILE: u16 = 266;
                    let zero_based = sprite.saturating_sub(1);
                    let file_index = zero_based / ITEMS_PER_FILE + 1;
                    let index_in_file = (zero_based % ITEMS_PER_FILE) as usize;
                    IconLoadPlan {
                        epf_path: format!("Legend/item{:03}.epf.bin", file_index),
                        palette_path: "Legend/item.ktx2".to_string(),
                        palette_index: self
                            .item_palette_table
                            .get(&sprite)
                            .copied()
                            .unwrap_or_default() as usize,
                        frame_index: index_in_file,
                    }
                }
                IconKind::Skill => IconLoadPlan {
                    epf_path: "setoa/skill001.epf.bin".to_string(),
                    palette_path: "setoa/gui.ktx2".to_string(),
                    palette_index: 6,
                    frame_index: sprite as usize,
                },
                IconKind::Spell => IconLoadPlan {
                    epf_path: "setoa/spell001.epf.bin".to_string(),
                    palette_path: "setoa/gui.ktx2".to_string(),
                    palette_index: 6,
                    frame_index: sprite as usize,
                },
            })
            .collect();

        // Collect every distinct archive path (EPF files + palettes) so the
        // whole batch is read in one parallel pass.
        let mut paths: Vec<String> = Vec::new();
        let mut path_index: HashMap<&str, usize> = HashMap::new();
        for plan in &plans {
            for path in [&plan.epf_path, &plan.palette_path] {
                if let std::collections::hash_map::Entry::Vacant(entry) = path_index.entry(path) {
                    entry.insert(paths.len());
                    paths.push(path.clone());
                }
            }
        }

        let file_results = game_files.get_files_parallel(&paths);
        let mut files: HashMap<String, Result<Vec<u8>, ArxError>> =
            HashMap::with_capacity(paths.len());
        for (path, result) in paths.into_iter().zip(file_results) {
            files.insert(path, result);
        }

        // Decode each palette file once (shared across every icon in the batch);
        // individual icons slice out the palette index they need below.
        let mut palettes: HashMap<String, Result<Vec<u8>, String>> = HashMap::new();
        for plan in &plans {
            if palettes.contains_key(&plan.palette_path) {
                continue;
            }
            let palette = match files.get(&plan.palette_path) {
                Some(Ok(bytes)) => SlintAssetLoader::decode_palette_file(bytes),
                Some(Err(err)) => Err(format!("{}: {}", plan.palette_path, err)),
                None => Err(format!("{} not found", plan.palette_path)),
            };
            palettes.insert(plan.palette_path.clone(), palette);
        }

        // Decode each EPF file once, in parallel.
        let epf_paths: Vec<String> = plans
            .iter()
            .map(|plan| plan.epf_path.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let worker_count = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .max(1);

        let mut epf_images: HashMap<String, Result<EpfImage, String>> =
            HashMap::with_capacity(epf_paths.len());
        for (_, (path, decoded)) in parallel_indexed(epf_paths.len(), worker_count, |index| {
            let path = &epf_paths[index];
            let decoded = match files.get(path) {
                Some(Ok(bytes)) => SlintAssetLoader::decode_epf_image(bytes),
                Some(Err(err)) => Err(format!("{}: {}", path, err)),
                None => Err(format!("{} not found", path)),
            };
            (path.clone(), decoded)
        }) {
            epf_images.insert(path, decoded);
        }

        // Build the final RGBA pixel buffers for every request, in parallel.
        let mut buffers: Vec<Option<Result<SharedPixelBuffer<Rgba8Pixel>, String>>> =
            vec![None; plans.len()];
        for (index, result) in
            parallel_indexed(plans.len(), worker_count.min(plans.len()).max(1), |index| {
                let plan = &plans[index];
                Self::build_icon_buffer(plan, &epf_images, &palettes)
            })
        {
            buffers[index] = Some(result);
        }

        buffers
            .into_iter()
            .map(|result| result.expect("every icon request filled"))
            .collect()
    }

    fn build_icon_buffer(
        plan: &IconLoadPlan,
        epf_images: &HashMap<String, Result<EpfImage, String>>,
        palettes: &HashMap<String, Result<Vec<u8>, String>>,
    ) -> Result<SharedPixelBuffer<Rgba8Pixel>, String> {
        let epf = match epf_images.get(&plan.epf_path) {
            Some(Ok(image)) => image,
            Some(Err(err)) => return Err(format!("{}: {}", plan.epf_path, err)),
            None => return Err(format!("{} not read", plan.epf_path)),
        };
        let palette = match palettes.get(&plan.palette_path) {
            Some(Ok(bytes)) => bytes,
            Some(Err(err)) => return Err(format!("{}: {}", plan.palette_path, err)),
            None => return Err(format!("{} not read", plan.palette_path)),
        };

        const PALETTE_SIZE: usize = 4 * 256;
        let offset = PALETTE_SIZE * plan.palette_index;
        let palette_rgba = palette.get(offset..offset + PALETTE_SIZE).ok_or_else(|| {
            format!(
                "palette index {} out of range (total {})",
                plan.palette_index,
                palette.len() / PALETTE_SIZE
            )
        })?;

        SlintAssetLoader::build_frame_buffer(epf, plan.frame_index, palette_rgba)
    }

    fn decode_palette_file(palette_bytes: &[u8]) -> Result<Vec<u8>, String> {
        if palette_bytes.is_empty() {
            return Err("Palette file is empty".to_string());
        }
        let (_, _, pal_data) = rendering::texture::Texture::load_ktx2(palette_bytes)
            .map_err(|e| format!("palette load: {e}"))?;
        Ok(pal_data)
    }

    fn decode_epf_image(epf_bytes: &[u8]) -> Result<EpfImage, String> {
        if epf_bytes.is_empty() {
            return Err("EPF file is empty".to_string());
        }
        let (epf_image, _): (EpfImage, _) =
            oxicode::decode_from_slice(epf_bytes).map_err(|e| format!("decode epf: {e}"))?;
        Ok(epf_image)
    }

    fn build_frame_buffer(
        epf_image: &EpfImage,
        frame_index: usize,
        palette_rgba: &[u8],
    ) -> Result<SharedPixelBuffer<Rgba8Pixel>, String> {
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

        Ok(pixel_buffer)
    }
}
