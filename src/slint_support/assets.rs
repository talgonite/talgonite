use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use formats::epf::AnimationDirection;
use formats::game_files::SquashfsError;
use formats::mpf::MpfAnimationType;
use formats::sheets::{CreatureSheet, ItemSheet, SheetFrame};
use formats::util::parallel_indexed;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use tracing::debug;

use crate::game_files::GameFiles;
use crate::metafile_store::MetafileStore;

/// Which icon sheet a sprite id refers to. Icons with the same kind share the
/// same packed sheet layout and palette, so sprite ids are only meaningful
/// within a kind.
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

/// One icon in a batch, with the exact sheet + frame + palette it needs.
struct IconLoadPlan {
    /// Base path of the packed sheet, without extension (e.g.
    /// `Legend/item001`). The runtime reads the single `{base}.sheet.bin`
    /// file (oxicode metadata + raw chunk pixels), exactly like the scene
    /// stores.
    sheet_path: String,
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
                "NPC {} not found in portrait map, falling back to creature sheet",
                npc_name
            );
        }

        let base = format!("hades/mns{:03}", sprite_id);
        let (sheet, chunk_pixels) = Self::load_sheet::<CreatureSheet>(game_files, &base, 1)?;

        let frame_index = if let Some(anim) = sheet
            .animations
            .iter()
            .find(|a| a.animation_type == MpfAnimationType::Standing)
        {
            anim.frame_index_for_direction(AnimationDirection::Towards) as usize
        } else {
            0
        };

        let frame = sheet
            .frames
            .get(frame_index)
            .copied()
            .flatten()
            .ok_or_else(|| format!("Frame index {} out of range", frame_index))?;

        let palette_path = "hades/mns.ktx2";
        let palette_bytes = game_files
            .get_file(palette_path)
            .ok_or_else(|| format!("Palette not found: {}", palette_path))?;
        let (_, _, pal_data) = rendering::texture::Texture::load_ktx2(&palette_bytes)
            .map_err(|e| format!("palette load: {e}"))?;

        let palette_rgba = Self::palette_row(&pal_data, sheet.palette_number as usize)?;
        let chunk = sheet
            .chunks
            .get(frame.chunk as usize)
            .ok_or_else(|| format!("chunk {} out of range", frame.chunk))?;
        let pixels = chunk_pixels
            .get(frame.chunk as usize)
            .ok_or_else(|| format!("chunk {} pixels missing", frame.chunk))?;
        let buffer = Self::bake_indexed_rect(
            pixels,
            chunk.width,
            frame.x,
            frame.y,
            frame.width,
            frame.height,
            palette_rgba,
        )?;
        Ok(Image::from_rgba8(buffer))
    }

    pub fn load_world_map_image(
        &self,
        game_files: &GameFiles,
        field_name: &str,
    ) -> Result<Image, String> {
        let base = format!("setoa/{}", field_name);
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

        let (sheet, chunk_pixels) = Self::load_sheet::<ItemSheet>(game_files, &base, 1)?;
        let frame = sheet
            .frames
            .first()
            .copied()
            .flatten()
            .ok_or_else(|| "world map sheet has no frames".to_string())?;
        let chunk = sheet
            .chunks
            .get(frame.chunk as usize)
            .ok_or_else(|| format!("chunk {} out of range", frame.chunk))?;
        let pixels = chunk_pixels
            .get(frame.chunk as usize)
            .ok_or_else(|| format!("chunk {} pixels missing", frame.chunk))?;
        let buffer = Self::bake_indexed_rect(
            pixels,
            chunk.width,
            frame.x,
            frame.y,
            frame.width,
            frame.height,
            &palette_rgba,
        )?;
        Ok(Image::from_rgba8(buffer))
    }

    /// Load `requests` (assumed deduplicated) in parallel and return a result
    /// per request. Sheet metadata, chunk images, and palettes are read
    /// through the archive's parallel reader, then rect extraction and pixel
    /// expansion are spread across worker threads.
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
                        sheet_path: format!("Legend/item{:03}", file_index),
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
                    sheet_path: "setoa/skill001".to_string(),
                    palette_path: "setoa/gui.ktx2".to_string(),
                    palette_index: 6,
                    frame_index: sprite as usize,
                },
                IconKind::Spell => IconLoadPlan {
                    sheet_path: "setoa/spell001".to_string(),
                    palette_path: "setoa/gui.ktx2".to_string(),
                    palette_index: 6,
                    frame_index: sprite as usize,
                },
            })
            .collect();

        // Collect every distinct palette path so palettes are read in one
        // parallel pass and shared across every icon in the batch.
        let mut paths: Vec<String> = Vec::new();
        let mut path_index: HashMap<String, usize> = HashMap::new();
        for plan in &plans {
            if let std::collections::hash_map::Entry::Vacant(entry) =
                path_index.entry(plan.palette_path.clone())
            {
                entry.insert(paths.len());
                paths.push(plan.palette_path.clone());
            }
        }

        let file_results = game_files.get_files_parallel(&paths);
        let mut files: HashMap<String, Result<Vec<u8>, SquashfsError>> =
            HashMap::with_capacity(paths.len());
        for (path, result) in paths.into_iter().zip(file_results) {
            files.insert(path, result);
        }

        // Decode each palette file once; individual icons slice out the
        // palette index they need below.
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

        // Decode each sheet (metadata + chunk images) once, in parallel.
        let sheet_bases: Vec<String> = plans
            .iter()
            .map(|plan| plan.sheet_path.clone())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let worker_count = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .max(1);

        let mut sheets: HashMap<String, Result<(ItemSheet, Vec<Vec<u8>>), String>> =
            HashMap::with_capacity(sheet_bases.len());
        for (_, (base, decoded)) in parallel_indexed(sheet_bases.len(), worker_count, |index| {
            let base = &sheet_bases[index];
            let decoded = Self::load_sheet::<ItemSheet>(game_files, base, 1);
            (base.clone(), decoded)
        }) {
            sheets.insert(base, decoded);
        }

        // Build the final RGBA pixel buffers for every request, in parallel.
        let mut buffers: Vec<Option<Result<SharedPixelBuffer<Rgba8Pixel>, String>>> =
            vec![None; plans.len()];
        for (index, result) in
            parallel_indexed(plans.len(), worker_count.min(plans.len()).max(1), |index| {
                let plan = &plans[index];
                Self::build_icon_buffer(plan, &sheets, &palettes)
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
        sheets: &HashMap<String, Result<(ItemSheet, Vec<Vec<u8>>), String>>,
        palettes: &HashMap<String, Result<Vec<u8>, String>>,
    ) -> Result<SharedPixelBuffer<Rgba8Pixel>, String> {
        let (sheet, chunk_pixels) = match sheets.get(&plan.sheet_path) {
            Some(Ok(sheet)) => sheet,
            Some(Err(err)) => return Err(format!("{}: {}", plan.sheet_path, err)),
            None => return Err(format!("{} not read", plan.sheet_path)),
        };
        let palette = match palettes.get(&plan.palette_path) {
            Some(Ok(bytes)) => bytes,
            Some(Err(err)) => return Err(format!("{}: {}", plan.palette_path, err)),
            None => return Err(format!("{} not read", plan.palette_path)),
        };

        let palette_rgba = Self::palette_row(palette, plan.palette_index)?;
        let frame = sheet
            .frames
            .get(plan.frame_index)
            .copied()
            .flatten()
            .ok_or_else(|| format!("frame index {} out of range", plan.frame_index))?;
        Self::bake_sheet_frame(frame, sheet, chunk_pixels, palette_rgba)
    }

    fn decode_palette_file(palette_bytes: &[u8]) -> Result<Vec<u8>, String> {
        if palette_bytes.is_empty() {
            return Err("Palette file is empty".to_string());
        }
        let (_, _, pal_data) = rendering::texture::Texture::load_ktx2(palette_bytes)
            .map_err(|e| format!("palette load: {e}"))?;
        Ok(pal_data)
    }

    /// Reads a packed sheet's single file (`{base}.sheet.bin`, oxicode
    /// metadata + raw chunk pixels). The metadata layout is shared with the
    /// scene stores, so the UI addresses frames through the exact same index.
    fn load_sheet<M: oxicode::Decode + formats::sheets::SheetMeta>(
        game_files: &GameFiles,
        base: &str,
        bytes_per_pixel: u32,
    ) -> Result<(M, Vec<Vec<u8>>), String> {
        let meta_path = format!("{base}.sheet.bin");
        let meta_bytes = game_files
            .get_file(&meta_path)
            .ok_or_else(|| format!("{} not found", meta_path))?;
        formats::sheets::decode_sheet::<M>(&meta_bytes, bytes_per_pixel)
            .map_err(|e| format!("decode sheet: {e}"))
    }

    /// Bakes one frame rect out of an `ItemSheet` chunk into an RGBA buffer.
    fn bake_sheet_frame(
        frame: SheetFrame,
        sheet: &ItemSheet,
        chunk_pixels: &[Vec<u8>],
        palette_rgba: &[u8],
    ) -> Result<SharedPixelBuffer<Rgba8Pixel>, String> {
        let chunk = sheet
            .chunks
            .get(frame.chunk as usize)
            .ok_or_else(|| format!("chunk {} out of range", frame.chunk))?;
        let pixels = chunk_pixels
            .get(frame.chunk as usize)
            .ok_or_else(|| format!("chunk {} pixels missing", frame.chunk))?;
        Self::bake_indexed_rect(
            pixels,
            chunk.width,
            frame.x,
            frame.y,
            frame.width,
            frame.height,
            palette_rgba,
        )
    }

    /// Bakes palette-indexed pixels for a rect inside a row-major chunk
    /// buffer into an RGBA buffer. Index 0 is transparent; other indices look
    /// up `palette_rgba` (one 256-color row per palette).
    fn bake_indexed_rect(
        chunk_pixels: &[u8],
        chunk_width: u32,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        palette_rgba: &[u8],
    ) -> Result<SharedPixelBuffer<Rgba8Pixel>, String> {
        let w = width as usize;
        let h = height as usize;
        let chunk_width = chunk_width as usize;
        let mut pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::new(width, height);
        let pixels = pixel_buffer.make_mut_slice();

        for row in 0..h {
            let row_base = (y as usize + row) * chunk_width + x as usize;
            for col in 0..w {
                let idx = *chunk_pixels
                    .get(row_base + col)
                    .ok_or_else(|| "chunk pixel data truncated".to_string())?;
                let dst = row * w + col;
                if idx == 0 {
                    pixels[dst] = Rgba8Pixel {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 0,
                    };
                } else {
                    let pal_idx = idx as usize * 4;
                    pixels[dst] = Rgba8Pixel {
                        r: palette_rgba[pal_idx],
                        g: palette_rgba[pal_idx + 1],
                        b: palette_rgba[pal_idx + 2],
                        a: palette_rgba[pal_idx + 3],
                    };
                }
            }
        }

        Ok(pixel_buffer)
    }

    /// Returns the 256-color RGBA row for `palette_index` from a packed
    /// palette texture (each row is 4 * 256 bytes).
    fn palette_row<'a>(palette: &'a [u8], palette_index: usize) -> Result<&'a [u8], String> {
        const PALETTE_SIZE: usize = 4 * 256;
        let offset = PALETTE_SIZE * palette_index;
        palette.get(offset..offset + PALETTE_SIZE).ok_or_else(|| {
            format!(
                "palette index {} out of range (total {})",
                palette_index,
                palette.len() / PALETTE_SIZE
            )
        })
    }
}
