use crate::{
    Instance, InstanceBatch,
    instance::InstanceFlag,
    make_quad,
    scene::{map::door_data::DOOR_DATA, utils::calculate_tile_z},
};
use etagere::Allocation;
use glam::Vec2;
use std::collections::{HashMap, HashSet};
use wgpu;

use crate::{
    scene::{
        AnimationInstanceData, InstanceReference, TILE_HEIGHT, TILE_WIDTH, TILEMAP_COLUMNS,
        TILEMAP_HEIGHT, TILEMAP_PAGE_HEIGHT, TILEMAP_PAGE_WIDTH, TILEMAP_TILE_HEIGHT,
        TILEMAP_TILE_WIDTH, TILEMAP_TILES_PER_PAGE_ROWS, TILEMAP_TILES_PER_ROW, WALL_ATLAS_HEIGHT,
        WALL_ATLAS_WIDTH, WorldAnimation, WorldAnimationInstanceData, Z_FLOOR, Z_WALLS,
        map::{
            animated_palette::AnimatedPaletteTexture,
            floor::FloorTile,
            map_tile::MapTile,
            wall::{Wall, WallSide},
        },
        texture_bind::TextureBind,
    },
    texture,
};
use formats::palette::AnimatedPaletteRange;

#[cfg(not(target_arch = "wasm32"))]
use formats::game_files::ArxArchive;
#[cfg(target_arch = "wasm32")]
use formats::game_files::WebArchive;

#[cfg(not(target_arch = "wasm32"))]
type Archive = ArxArchive;
#[cfg(target_arch = "wasm32")]
type Archive = WebArchive;

/// Palette textures store one 256-color palette per row, matching the
/// `palette_offset / 256.0` normalization used by the map shader.
const PALETTE_TEXTURE_WIDTH: u32 = 256;

#[cfg(not(target_arch = "wasm32"))]
fn get_files_or_panic<S>(archive: &Archive, paths: &[S]) -> Vec<Vec<u8>>
where
    S: AsRef<str> + Sync,
{
    archive
        .get_files_parallel(paths)
        .into_iter()
        .zip(paths.iter())
        .map(|(result, path)| match result {
            Ok(bytes) => bytes,
            Err(error) => panic!("Failed to get file '{}': {}", path.as_ref(), error),
        })
        .collect()
}

fn load_animated_palette_table(
    archive: &Archive,
    path: &str,
) -> Vec<(u16, Vec<AnimatedPaletteRange>)> {
    match archive.get_file(path) {
        Ok(bytes) => {
            let decoded =
                oxicode::decode_from_slice::<formats::palette::AnimatedPaletteTable>(&bytes);

            match decoded {
                Ok((table, _)) => table.entries,
                Err(error) => {
                    tracing::warn!(
                        path,
                        %error,
                        "Failed to decode map animated palette table"
                    );
                    Vec::new()
                }
            }
        }
        Err(error) => {
            tracing::warn!(
                path,
                %error,
                "Animated palette table missing; map palette animations disabled \
                 (re-run the installer to regenerate game data)"
            );
            Vec::new()
        }
    }
}

/// True when the foreground tile (wall) id should render with screen blending.
/// sotp.dat stores one byte per tile id - 1; bit 0x80 is TileFlags.Transparent,
/// which the reference client composites with blend mode 0x6D.
fn is_screen_blend_wall(sotp_data: &[u8], wall_id: u16) -> bool {
    sotp_data
        .get((wall_id as usize).saturating_sub(1))
        .is_some_and(|&byte| byte & 0x80 != 0)
}

pub struct PreparedMap {
    pub tile_texture_data: Vec<u8>,
    pub palette_texture_data: Vec<u8>,
    pub palette_pixels: Vec<u8>,
    pub wall_palette_data: Vec<u8>,
    pub wall_palette_pixels: Vec<u8>,
    pub wall_map_buf: Vec<u8>,
    pub wall_heights: HashMap<u16, u16>,
    pub screen_blend_wall_ids: Vec<u16>,
    pub animations: Vec<WorldAnimationInstanceData>,
    pub wall_toggle_animations: HashMap<(u8, u8), AnimationInstanceData>,
    pub wall_toggle_tracker: HashMap<(u16, usize), ((u8, u8), Vec<Instance>)>,
    pub floor_animated_palettes: Vec<(u16, Vec<AnimatedPaletteRange>)>,
    pub wall_animated_palettes: Vec<(u16, Vec<AnimatedPaletteRange>)>,
    wall_animations: Vec<WorldAnimationInstanceData>,
    tile_instances: Vec<Instance>,
    allocated: HashMap<u16, (Allocation, Vec<Instance>)>,
}

pub struct MapRenderer {
    animations: Vec<WorldAnimationInstanceData>,
    wall_toggle_animations: HashMap<(u8, u8), AnimationInstanceData>,
    instance_batches: Vec<InstanceBatch>,
    screen_blend_batch_indices: Vec<usize>,
    wall_animated_palettes: Option<AnimatedPaletteTexture>,
    floor_animated_palettes: Option<AnimatedPaletteTexture>,
}

impl MapRenderer {
    pub fn empty() -> Self {
        Self {
            instance_batches: Vec::new(),
            animations: Vec::new(),
            wall_toggle_animations: HashMap::new(),
            screen_blend_batch_indices: Vec::new(),
            wall_animated_palettes: None,
            floor_animated_palettes: None,
        }
    }

    pub fn new(
        instance_batches: Vec<InstanceBatch>,
        animations: Vec<WorldAnimationInstanceData>,
        wall_toggle_animations: HashMap<(u8, u8), AnimationInstanceData>,
        screen_blend_batch_indices: Vec<usize>,
        wall_animated_palettes: Option<AnimatedPaletteTexture>,
        floor_animated_palettes: Option<AnimatedPaletteTexture>,
    ) -> Self {
        Self {
            instance_batches,
            animations,
            wall_toggle_animations,
            screen_blend_batch_indices,
            wall_animated_palettes,
            floor_animated_palettes,
        }
    }

    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        for (batch_index, batch) in self.instance_batches.iter().enumerate() {
            if self.screen_blend_batch_indices.contains(&batch_index) {
                continue;
            }
            batch.draw(render_pass);
        }
    }

    /// Whether any wall batches need the screen-blend pipeline.
    pub fn has_screen_blend_walls(&self) -> bool {
        !self.screen_blend_batch_indices.is_empty()
    }

    /// Draws only the walls whose sotp.dat byte has the Transparent flag (0x80).
    /// These must be drawn with the screen-blend pipeline after the opaque scene.
    pub fn render_screen_blend<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        for &batch_index in &self.screen_blend_batch_indices {
            if let Some(batch) = self.instance_batches.get(batch_index) {
                batch.draw(render_pass);
            }
        }
    }

    /// Compute 2D bounds (min, max) of all tile & wall instances in screen space.
    /// Returns None if there are no instances.
    pub fn bounds(&self) -> Option<(glam::Vec2, glam::Vec2)> {
        let mut min: Option<glam::Vec2> = None;
        let mut max: Option<glam::Vec2> = None;
        for (batch_index, batch) in self.instance_batches.iter().enumerate() {
            // First batch is floor tiles (quads of TILE_WIDTH x TILE_HEIGHT). Others are walls whose
            // quad size differs. We expand bounds by their quad vertex size where possible.
            let (quad_w, quad_h) = if batch_index == 0 {
                (
                    crate::scene::TILE_WIDTH as f32,
                    crate::scene::TILE_HEIGHT as f32,
                )
            } else {
                // Walls: width is 28, height varies – we approximate height from first instance delta in tex coords * atlas size.
                // Fallback: just use tile width/height minima so map never collapses.
                (28.0_f32, crate::scene::TILE_HEIGHT as f32)
            };
            for inst in &batch.instances {
                let p = inst.position.truncate();
                let tl = p; // instance positions are already top-left for floors, adjusted for walls.
                let br = p + glam::Vec2::new(quad_w, quad_h);
                min = Some(match min {
                    Some(m) => m.min(tl),
                    None => tl,
                });
                max = Some(match max {
                    Some(m) => m.max(br),
                    None => br,
                });
            }
        }
        match (min, max) {
            (Some(a), Some(b)) => Some((a, b)),
            _ => None,
        }
    }

    /// Returns floor tile instances (batch 0) for CPU fallback visualization.
    pub fn floor_instances(&self) -> &[crate::Instance] {
        self.instance_batches
            .get(0)
            .map(|b| b.instances.as_slice())
            .unwrap_or(&[])
    }

    pub fn update_animations(&mut self, queue: &wgpu::Queue) {
        let now = std::time::Instant::now();

        if let Some(animated_palettes) = &mut self.wall_animated_palettes {
            animated_palettes.update(queue, now);
        }

        if let Some(animated_palettes) = &mut self.floor_animated_palettes {
            animated_palettes.update(queue, now);
        }

        for anim in &mut self.animations {
            if !anim.should_update(now) {
                continue;
            }

            let changes_to_apply = anim.advance(now);

            for instance_ref in anim.instances().iter() {
                if let Some(batch) = self.instance_batches.get_mut(instance_ref.batch_index) {
                    if let Some(instance) = batch.get_instance(instance_ref.instance_index) {
                        batch.update_instance(
                            queue,
                            instance_ref.instance_index,
                            Instance {
                                position: instance.position + changes_to_apply.position,
                                tex_min: changes_to_apply.tex_min,
                                tex_max: changes_to_apply.tex_max,
                                sprite_size: changes_to_apply.sprite_size,
                                palette_offset: changes_to_apply.palette_offset,
                                dye_v_offset: instance.dye_v_offset,
                                flags: instance.flags,
                                tint: instance.tint,
                            },
                        );
                    }
                }
            }
        }
    }

    pub fn set_wall_toggle_state(&mut self, queue: &wgpu::Queue, x: u8, y: u8, state: bool) {
        if let Some(anim) = self.wall_toggle_animations.get_mut(&(x, y)) {
            let frame = if state { 1 } else { 0 };
            let instance_data = anim.set_frame(frame);

            for instance_ref in &anim.instances {
                if let Some(batch) = self.instance_batches.get_mut(instance_ref.batch_index) {
                    if let Some(instance) = batch.get_instance(instance_ref.instance_index) {
                        batch.update_instance(
                            queue,
                            instance_ref.instance_index,
                            Instance {
                                position: instance.position + instance_data.position,
                                tex_min: instance_data.tex_min,
                                tex_max: instance_data.tex_max,
                                sprite_size: instance_data.sprite_size,
                                palette_offset: instance_data.palette_offset,
                                dye_v_offset: instance.dye_v_offset,
                                flags: instance.flags,
                                tint: instance.tint,
                            },
                        );
                    }
                }
            }
        } else {
            tracing::warn!("No wall toggle animation found at ({}, {})", x, y);
        }
    }

    pub fn prepare_map(
        archive: &Archive,
        map_data: Vec<u8>,
        map_width: u8,
        map_height: u8,
        is_snow: bool,
        xray: bool,
    ) -> PreparedMap {
        let mpt_data = archive.get_file_or_panic("seo/mpt.tbl.bin");

        let (tile_palette_table, _): (rangemap::RangeMap<u16, u16>, usize) =
            oxicode::serde::decode_from_slice(&mpt_data, oxicode::config::standard()).unwrap();

        let wall_table_name = format!("ia/st{}.tbl.bin", if is_snow { "s" } else { "c" });
        let wall_table_data = archive.get_file_or_panic(&wall_table_name);

        let (wall_palette_table, _): (rangemap::RangeMap<u16, u16>, usize) =
            oxicode::serde::decode_from_slice(&wall_table_data, oxicode::config::standard())
                .unwrap();

        let wall_animated_palette_table = load_animated_palette_table(
            archive,
            &format!("ia/st{}.anipal.tbl.bin", if is_snow { "s" } else { "c" }),
        );
        let floor_animated_palette_table =
            load_animated_palette_table(archive, "seo/mpt.anipal.tbl.bin");

        // sotp.dat: one byte per foreground tile id - 1. Bit 0x0F is TileFlags.Wall
        // (collision) and bit 0x80 is TileFlags.Transparent, which the reference
        // client renders with screen blending (blend mode 0x6D).
        let sotp_data = match archive.get_file("ia/sotp.dat") {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!(
                    %error,
                    "sotp.dat missing; transparent-wall screen blending disabled"
                );
                Vec::new()
            }
        };

        let build_tile_instance = |floor: &FloorTile, x, y| -> Instance {
            let tile_id = floor.tile_id() as u32;
            let palette_offset = tile_palette_table.get(&floor.palette_index()).unwrap_or(&0);

            let tilemap_y = tile_id / TILEMAP_COLUMNS;
            let tilemap_x = tile_id - (tilemap_y * TILEMAP_COLUMNS);
            let coord = FloorTile::get_position(x as f32, y as f32);
            Instance {
                position: coord.extend(Z_FLOOR),
                tex_min: Vec2::new(
                    tilemap_x as f32 * TILEMAP_TILE_WIDTH,
                    tilemap_y as f32 * TILEMAP_TILE_HEIGHT,
                ),
                tex_max: Vec2::new(
                    (tilemap_x as f32 + 1.0) * TILEMAP_TILE_WIDTH,
                    (tilemap_y as f32 + 1.0) * TILEMAP_TILE_HEIGHT,
                ),
                sprite_size: Vec2::new(1., 1.),
                palette_offset: (*palette_offset as f32) / 256.0,
                dye_v_offset: -1.,
                flags: InstanceFlag::None,
                tint: glam::Vec3::ZERO,
            }
        };

        let (required_wall_ids, active_wall_animations, map_data) = {
            let mut map_reader = std::io::Cursor::new(map_data);
            let mut required_wall_ids: Vec<u16> = Vec::new();

            for _ in 0..((map_width as usize) * (map_height as usize)) {
                let tile = MapTile::read_from_reader(&mut map_reader);

                let walls = [tile.wall_left, tile.wall_right];

                for wall in walls.iter().filter(|w| w.show()) {
                    if !required_wall_ids.contains(&wall.id) {
                        required_wall_ids.push(wall.id);
                    }
                }
            }
            let wall_anim_data = archive.get_file_or_panic("ia/stcani.tbl");

            let wall_anim_store =
                WorldAnimation::from_string(&String::from_utf8_lossy(&wall_anim_data));

            let mut matching_wall_anims: Vec<WorldAnimation> = Vec::new();

            for anim in wall_anim_store {
                if anim.ids.iter().any(|id| required_wall_ids.contains(id)) {
                    matching_wall_anims.push(anim);
                }
            }

            let door_pairs = crate::scene::map::door_data::get_door_tile_toggle_pairs();
            let mut door_lookup = HashMap::new();
            for pair in &door_pairs {
                door_lookup.insert(pair.open_tile, pair.closed_tile);
                door_lookup.insert(pair.closed_tile, pair.open_tile);
            }

            for door_id in DOOR_DATA.iter() {
                if required_wall_ids.contains(door_id) {
                    let partner_id = door_lookup.get(door_id).unwrap();
                    if !required_wall_ids.contains(partner_id) {
                        required_wall_ids.push(*partner_id);
                    }
                }
            }

            for anim in &matching_wall_anims {
                for wall_id in &anim.ids {
                    if !required_wall_ids.contains(wall_id) {
                        required_wall_ids.push(*wall_id);
                    }
                }
            }

            (
                required_wall_ids,
                matching_wall_anims,
                map_reader.into_inner(),
            )
        };

        let screen_blend_wall_ids: Vec<u16> = required_wall_ids
            .iter()
            .copied()
            .filter(|&wall_id| is_screen_blend_wall(&sotp_data, wall_id))
            .collect();

        let used_wall_palette_rows: HashSet<u16> = required_wall_ids
            .iter()
            .map(|id| *wall_palette_table.get(&(id + 1)).unwrap_or(&0))
            .collect();
        let wall_animated_palettes: Vec<(u16, Vec<AnimatedPaletteRange>)> =
            wall_animated_palette_table
                .into_iter()
                .filter(|(row, _)| used_wall_palette_rows.contains(row))
                .collect();

        let door_pairs = crate::scene::map::door_data::get_door_tile_toggle_pairs();
        let mut wall_toggle_tracker: HashMap<(u16, usize), ((u8, u8), Vec<Instance>)> =
            HashMap::new();

        let (mut animations, map_data, floor_animated_palettes) = {
            let floor_anim_data = archive.get_file_or_panic("seo/gndani.tbl");

            let all_floor_animations =
                WorldAnimation::from_string(&String::from_utf8_lossy(&floor_anim_data));

            let mut map_reader = std::io::Cursor::new(map_data);

            let floors: HashMap<u16, FloorTile> = (0..((map_width as usize)
                * (map_height as usize)))
                .map(|_| MapTile::read_from_reader(&mut map_reader))
                .filter(|tile| tile.floor.show())
                .map(|tile| (tile.floor.id, tile.floor))
                .collect();

            let tile_animations: Vec<WorldAnimationInstanceData> = all_floor_animations
                .into_iter()
                .filter(|anim| anim.ids.iter().any(|id| floors.contains_key(id)))
                .map(|anim| {
                    let frames: Vec<Instance> = anim
                        .ids
                        .iter()
                        .map(|id| {
                            let tile = floors.get(id).unwrap();
                            build_tile_instance(tile, 0, 0) // These positions will be converted into offsets in the animation
                        })
                        .collect();

                    WorldAnimationInstanceData::new(anim.clone(), frames)
                })
                .collect();

            let used_floor_palette_rows: HashSet<u16> = floors
                .keys()
                .map(|id| *tile_palette_table.get(&(id + 1)).unwrap_or(&0))
                .collect();
            let floor_animated_palettes: Vec<(u16, Vec<AnimatedPaletteRange>)> =
                floor_animated_palette_table
                    .into_iter()
                    .filter(|(row, _)| used_floor_palette_rows.contains(row))
                    .collect();

            (
                tile_animations,
                map_reader.into_inner(),
                floor_animated_palettes,
            )
        };

        // Determine required floor page indices from map tiles and animations
        let mut map_reader_for_pages = std::io::Cursor::new(&map_data);
        let mut needed_pages: HashSet<usize> = HashSet::new();
        let tiles_per_page: usize = (TILEMAP_TILES_PER_ROW * TILEMAP_TILES_PER_PAGE_ROWS) as usize;
        for _ in 0..((map_width as usize) * (map_height as usize)) {
            let tile = MapTile::read_from_reader(&mut map_reader_for_pages);
            if tile.floor.show() {
                let tile_id = tile.floor.tile_id() as usize;
                needed_pages.insert(tile_id / tiles_per_page);
            }
        }
        // Include pages for any animated frames (ids are guaranteed to be in map per existing logic)
        // The existing animations builder only includes anims whose ids are present in floors.
        // So the pass above already marked those pages.

        // Clone map_data for the first reading pass
        let mut map_reader = std::io::Cursor::new(map_data);

        let mut wall_map_buf = vec![0u8; WALL_ATLAS_WIDTH * WALL_ATLAS_HEIGHT];
        let mut tile_instances: Vec<Instance> = Vec::new();
        let mut atlas = etagere::AtlasAllocator::with_options(
            etagere::size2(WALL_ATLAS_WIDTH as i32, WALL_ATLAS_HEIGHT as i32),
            &etagere::AllocatorOptions {
                num_columns: WALL_ATLAS_WIDTH as i32 / 28,
                ..Default::default()
            },
        );

        let mut allocated: HashMap<u16, (etagere::Allocation, Vec<Instance>)> = HashMap::new();
        let mut wall_heights: HashMap<u16, u16> = HashMap::new();

        {
            let wall_paths: Vec<String> = required_wall_ids
                .iter()
                .map(|wall| format!("ia/stc{:05}.ktx2", wall))
                .collect();
            let wall_bytes = get_files_or_panic(archive, &wall_paths);

            for (wall, bytes) in required_wall_ids.into_iter().zip(wall_bytes) {
                let reader = ktx2::Reader::new(bytes).unwrap();
                let info = reader.header();
                wall_heights.insert(wall, info.pixel_height as u16);
                let rounded_height = (info.pixel_height as u32 + 63) & !63;
                let a = atlas
                    .allocate(etagere::size2(28, rounded_height as i32))
                    .unwrap();
                allocated.insert(wall, (a, Vec::new()));

                let buf = reader.levels().nth(0).unwrap();
                assert!(buf.data.len() % 28 == 0);

                let mut offset =
                    a.rectangle.min.y as usize * WALL_ATLAS_WIDTH + a.rectangle.min.x as usize;

                let padding = rounded_height - info.pixel_height as u32;
                let padding_chunks = (0..padding).map(|_| [0; 28].as_slice());

                for chunk in padding_chunks.chain(buf.data.chunks(28)) {
                    wall_map_buf[offset..offset + 28].copy_from_slice(chunk);
                    offset += WALL_ATLAS_WIDTH;
                }
            }
        }
        let build_wall_instance = |wall: Wall, x: f32, y: f32, a: &Allocation| -> Instance {
            let height = a.rectangle.max.y - a.rectangle.min.y;

            let coord = wall.side.get_position(x, y, height as f32);
            let palette_offset = *wall_palette_table.get(&wall.palette_index()).unwrap_or(&0);
            Instance {
                position: coord.extend(calculate_tile_z(x, y, Z_WALLS)),
                tex_min: Vec2::new(
                    a.rectangle.min.x as f32 / WALL_ATLAS_WIDTH as f32,
                    a.rectangle.min.y as f32 / WALL_ATLAS_HEIGHT as f32,
                ),
                tex_max: Vec2::new(
                    (a.rectangle.min.x + 28) as f32 / WALL_ATLAS_WIDTH as f32,
                    (a.rectangle.min.y + height) as f32 / WALL_ATLAS_HEIGHT as f32,
                ),
                sprite_size: Vec2::new(1., 1.),
                palette_offset: palette_offset as f32 / 256.0,
                dye_v_offset: -1.,
                flags: if xray {
                    InstanceFlag::XRay
                } else {
                    InstanceFlag::None
                },
                tint: glam::Vec3::ZERO,
            }
        };

        for y in 0..map_height {
            for x in 0..map_width {
                let MapTile {
                    floor,
                    wall_left,
                    wall_right,
                } = MapTile::read_from_reader(&mut map_reader);

                for wall in [wall_left, wall_right] {
                    if !wall.show() {
                        continue;
                    }

                    // Change to the first frames wall ID if it matches an animation
                    // Otherwise the position offset will behave incorrectly
                    let wall_id = if let Some(anim) = active_wall_animations
                        .iter()
                        .find(|anim| anim.ids.contains(&wall.id))
                    {
                        anim.ids[0]
                    } else {
                        wall.id
                    };

                    let (a, instances) = allocated.get_mut(&wall_id).unwrap();
                    let instance_idx = instances.len();
                    instances.push(build_wall_instance(wall, x as f32, y as f32, a));

                    if let Some(pair) = door_pairs
                        .iter()
                        .find(|p| p.open_tile == wall.id || p.closed_tile == wall.id)
                    {
                        let open_wall = Wall {
                            id: pair.open_tile,
                            side: wall.side,
                        };
                        let closed_wall = Wall {
                            id: pair.closed_tile,
                            side: wall.side,
                        };

                        let (a_open, _) = allocated.get(&pair.open_tile).unwrap();
                        let (a_closed, _) = allocated.get(&pair.closed_tile).unwrap();

                        let open_instance =
                            build_wall_instance(open_wall, x as f32, y as f32, a_open);
                        let closed_instance =
                            build_wall_instance(closed_wall, x as f32, y as f32, a_closed);

                        wall_toggle_tracker.insert(
                            (wall_id, instance_idx),
                            ((x, y), vec![open_instance, closed_instance]),
                        );
                    }
                }

                if floor.show() {
                    let instance_idx = tile_instances.len();
                    tile_instances.push(build_tile_instance(&floor, x, y));

                    if let Some(anim) = animations
                        .iter_mut()
                        .find(|anim| anim.contains_id(floor.id))
                    {
                        anim.data.instances.push(InstanceReference {
                            batch_index: 0, // Tile animations are always in the first batch
                            instance_index: instance_idx,
                        });
                    }
                }
            }
        }

        let wall_animations: Vec<WorldAnimationInstanceData> = active_wall_animations
            .into_iter()
            .map(|anim| {
                let frames: Vec<Instance> = anim
                    .ids
                    .iter()
                    .map(|wall_id| {
                        let wall = Wall {
                            id: *wall_id,
                            side: WallSide::Left,
                        };
                        let (a, _) = allocated.get_mut(&wall.id).unwrap();
                        build_wall_instance(wall, 0., 0., a)
                    })
                    .collect();

                WorldAnimationInstanceData::new(anim, frames)
            })
            .collect();

        // Assemble floor tile atlas from required paged KTX2 files only
        let mut tile_texture_data =
            vec![0u8; (TILEMAP_PAGE_WIDTH as usize) * (TILEMAP_HEIGHT as usize)];
        let mut needed_pages_vec: Vec<usize> = needed_pages.into_iter().collect();
        needed_pages_vec.sort_unstable();
        let tile_page_paths: Vec<String> = needed_pages_vec
            .iter()
            .map(|page_index| format!("seo/tilea_{:03}.ktx2", page_index))
            .collect();
        let tile_page_bytes = get_files_or_panic(archive, &tile_page_paths);

        for (page_index, page_bytes) in needed_pages_vec.into_iter().zip(tile_page_bytes) {
            let (w, h, data) = texture::Texture::load_ktx2(&page_bytes).unwrap();
            debug_assert_eq!(w, TILEMAP_PAGE_WIDTH);
            let copy_height = h as usize;
            let dst_stride = TILEMAP_PAGE_WIDTH as usize;
            let src_stride = w as usize;
            let dst_y = page_index * (TILEMAP_PAGE_HEIGHT as usize);
            if dst_y + copy_height > TILEMAP_HEIGHT as usize {
                continue;
            }
            for row in 0..copy_height {
                let src_off = row * src_stride;
                let dst_off = (dst_y + row) * dst_stride;
                tile_texture_data[dst_off..dst_off + src_stride]
                    .copy_from_slice(&data[src_off..src_off + src_stride]);
            }
        }

        let palette_texture_data = archive.get_file_or_panic("seo/mpt.ktx2");
        let wall_palette_data = archive.get_file_or_panic("ia/stc.ktx2");
        let (_, _, palette_pixels) = texture::Texture::load_ktx2(&palette_texture_data)
            .expect("Failed to decode floor palette texture");
        let (_, _, wall_palette_pixels) = texture::Texture::load_ktx2(&wall_palette_data)
            .expect("Failed to decode wall palette texture");

        PreparedMap {
            tile_texture_data,
            palette_texture_data,
            palette_pixels,
            wall_palette_data,
            wall_palette_pixels,
            wall_map_buf,
            wall_heights,
            screen_blend_wall_ids,
            animations,
            wall_toggle_animations: HashMap::new(),
            wall_toggle_tracker,
            floor_animated_palettes,
            wall_animated_palettes,
            wall_animations,
            tile_instances,
            allocated,
        }
    }

    pub fn bind_map(device: &wgpu::Device, queue: &wgpu::Queue, map: PreparedMap) -> Self {
        let mut map = map;
        let diffuse_texture = texture::Texture::from_data(
            &device,
            &queue,
            "tile_atlas",
            TILEMAP_PAGE_WIDTH,
            TILEMAP_HEIGHT,
            wgpu::TextureFormat::R8Unorm,
            &map.tile_texture_data,
        )
        .unwrap();

        let palette_texture = texture::Texture::from_ktx2_rgba8(
            &device,
            &queue,
            "tile_palette",
            &map.palette_texture_data,
        )
        .unwrap();

        let floor_animated_palettes = AnimatedPaletteTexture::new(
            palette_texture,
            map.palette_pixels,
            PALETTE_TEXTURE_WIDTH,
            map.floor_animated_palettes,
            std::time::Instant::now(),
        );

        let tile_bind_group = TextureBind::to_bind_group(
            device,
            &diffuse_texture,
            &floor_animated_palettes.texture,
            &texture::Texture::empty_view(device, "tile_empty"),
        );

        let mut instance_batches: Vec<InstanceBatch> = Vec::new();

        instance_batches.push(InstanceBatch::new(
            &device,
            map.tile_instances,
            make_quad(TILE_WIDTH, TILE_HEIGHT).to_vec(),
            tile_bind_group,
        ));

        let diffuse_texture = texture::Texture::from_data(
            &device,
            &queue,
            "wall_atlas",
            WALL_ATLAS_WIDTH as u32,
            WALL_ATLAS_HEIGHT as u32,
            wgpu::TextureFormat::R8Unorm,
            &map.wall_map_buf,
        )
        .unwrap();

        let palette_texture = texture::Texture::from_ktx2_rgba8(
            &device,
            &queue,
            "wall_palette",
            &map.wall_palette_data,
        )
        .unwrap();

        let wall_animated_palettes = AnimatedPaletteTexture::new(
            palette_texture,
            map.wall_palette_pixels,
            PALETTE_TEXTURE_WIDTH,
            map.wall_animated_palettes,
            std::time::Instant::now(),
        );

        // find each different height allocated and create a batch for it
        // group the allocations by height so that they can allocate more tightly on the atlas.
        // Transparent (screen-blend) walls get their own batches so they can be drawn last
        // with the screen-blend pipeline.
        let mut height_map: HashMap<(i32, bool), Vec<(etagere::Allocation, u16, Vec<Instance>)>> =
            HashMap::new();

        for (wall_id, (a, instances)) in map.allocated {
            let screen_blend = map.screen_blend_wall_ids.contains(&wall_id);
            height_map
                .entry((a.rectangle.max.y - a.rectangle.min.y, screen_blend))
                .or_insert_with(Vec::new)
                .push((a, wall_id, instances));
        }

        let door_pairs = crate::scene::map::door_data::get_door_tile_toggle_pairs();
        let mut screen_blend_batch_indices: Vec<usize> = Vec::new();
        for ((height, screen_blend), instances_at_height) in height_map {
            let vertices = make_quad(28, height as u32).to_vec();

            let batch_index = instance_batches.len();
            if screen_blend {
                screen_blend_batch_indices.push(batch_index);
            }
            let mut instances: Vec<Instance> = Vec::new();

            for (_, wall_id, curr_instances) in instances_at_height {
                for (idx_in_wall_list, instance) in curr_instances.into_iter().enumerate() {
                    let instance_index = instances.len();

                    for anim in map.wall_animations.iter_mut() {
                        if anim.contains_id(wall_id) {
                            anim.data.instances.push(InstanceReference {
                                batch_index,
                                instance_index: instance_index,
                            });
                        }
                    }

                    if let Some(((x, y), frames)) =
                        map.wall_toggle_tracker.get(&(wall_id, idx_in_wall_list))
                    {
                        let mut anim = AnimationInstanceData::new(frames.clone());
                        anim.instances.push(InstanceReference {
                            batch_index,
                            instance_index,
                        });

                        // If the wall we found was the closed one, set frame to 1
                        // We can check this by looking at the door pairs
                        if door_pairs.iter().any(|p| p.closed_tile == wall_id) {
                            anim.frame = 1;
                        }

                        map.wall_toggle_animations.insert((*x, *y), anim);
                    }

                    instances.push(instance);
                }
            }

            let wall_bind_group = TextureBind::to_bind_group(
                device,
                &diffuse_texture,
                &wall_animated_palettes.texture,
                &texture::Texture::empty_view(device, "wall_empty"),
            );

            instance_batches.push(InstanceBatch::new(
                &device,
                instances,
                vertices,
                wall_bind_group,
            ));
        }

        map.animations.extend(map.wall_animations);

        MapRenderer::new(
            instance_batches,
            map.animations,
            map.wall_toggle_animations,
            screen_blend_batch_indices,
            Some(wall_animated_palettes),
            Some(floor_animated_palettes),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::is_screen_blend_wall;

    #[test]
    fn screen_blend_flag_matches_tile_flags_semantics() {
        // Real sotp.dat byte values: 0x00 = none, 0x0F = Wall, 0x80 = Transparent,
        // 0x8F = Wall | Transparent.
        let sotp = [0x00u8, 0x0F, 0x80, 0x8F];

        // sotp index = tile id - 1
        assert!(!is_screen_blend_wall(&sotp, 1)); // 0x00
        assert!(!is_screen_blend_wall(&sotp, 2)); // 0x0F (opaque wall)
        assert!(is_screen_blend_wall(&sotp, 3)); // 0x80 (fence/window)
        assert!(is_screen_blend_wall(&sotp, 4)); // 0x8F (wall + transparent)

        // id 0, ids past the table, and missing tables are never screen-blended
        assert!(!is_screen_blend_wall(&sotp, 0));
        assert!(!is_screen_blend_wall(&sotp, 5));
        assert!(!is_screen_blend_wall(&[], 3));
    }
}
