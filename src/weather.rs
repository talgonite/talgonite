//! Weather asset loading: bakes palette-indexed sheets to RGBA and packs the
//! snow atlas.

use formats::game_files::SquashfsArchive;
use formats::sheets::{ItemSheet, SheetFrame, decode_sheet};
use rendering::scene::weather::{SnowFrame, WeatherAssets, WeatherSprite};
use tracing::warn;

const SNOW_TYPES: [&str; 4] = ["snowa00", "snowa01", "snowa02", "snowa03"];

/// Loads the snow atlas and rain texture; `None` when assets are missing.
pub fn load_weather_assets(archive: &SquashfsArchive) -> Option<WeatherAssets> {
    let palette = load_palette(archive)?;
    let mut baked: Vec<(u32, u32, Vec<u8>)> = Vec::new();
    let mut type_ranges = Vec::new();

    for name in SNOW_TYPES {
        let Some((meta, chunks)) = load_sheet(archive, name) else {
            continue;
        };
        let start = baked.len();
        for frame in meta.frames.iter().flatten() {
            let Some(baked_frame) = bake_frame(&meta, &chunks, frame, &palette) else {
                continue;
            };
            baked.push(baked_frame);
        }
        if baked.len() > start {
            type_ranges.push((start, baked.len()));
        }
    }

    let (snow_atlas, packed_frames) = pack_atlas(&baked);
    let snow_frames = type_ranges
        .iter()
        .map(|&(start, end)| packed_frames[start..end].to_vec())
        .collect();

    let rain = load_sheet(archive, "rain01").and_then(|(meta, chunks)| {
        let frame = meta.frames.first().and_then(|f| f.as_ref())?;
        let (w, h, pixels) = bake_frame(&meta, &chunks, frame, &palette)?;
        Some(WeatherSprite {
            width: w,
            height: h,
            pixels,
        })
    })?;

    Some(WeatherAssets {
        snow_atlas,
        snow_frames,
        rain,
    })
}

fn load_sheet(archive: &SquashfsArchive, name: &str) -> Option<(ItemSheet, Vec<Vec<u8>>)> {
    let bytes = archive
        .get_file(&format!("Legend/{name}.sheet.bin"))
        .map_err(|error| warn!(name, %error, "Failed to load weather sheet"))
        .ok()?;
    match decode_sheet::<ItemSheet>(&bytes, 1) {
        Ok(sheet) => Some(sheet),
        Err(error) => {
            warn!(name, %error, "Failed to decode weather sheet");
            None
        }
    }
}

fn load_palette(archive: &SquashfsArchive) -> Option<[[u8; 3]; 256]> {
    let bytes = archive
        .get_file("Legend/legend01.pal")
        .map_err(|error| warn!(%error, "Failed to load legend01.pal for weather"))
        .ok()?;
    if bytes.len() < 768 {
        warn!("legend01.pal too short: {} bytes", bytes.len());
        return None;
    }

    let mut palette = [[0u8; 3]; 256];
    for (i, color) in palette.iter_mut().enumerate() {
        color.copy_from_slice(&bytes[i * 3..i * 3 + 3]);
    }
    Some(palette)
}

/// Bakes one sheet frame to RGBA, padding frames with negative origins.
fn bake_frame(
    meta: &ItemSheet,
    chunks: &[Vec<u8>],
    frame: &SheetFrame,
    palette: &[[u8; 3]; 256],
) -> Option<(u32, u32, Vec<u8>)> {
    if frame.width == 0 || frame.height == 0 {
        return None;
    }

    let pad_x = frame.left.max(0) as u32;
    let pad_y = frame.top.max(0) as u32;
    let width = frame.width + pad_x;
    let height = frame.height + pad_y;
    let chunk = chunks.get(frame.chunk as usize)?;
    let chunk_width = meta.chunks[frame.chunk as usize].width as usize;

    let mut rgba = vec![0u8; width as usize * height as usize * 4];
    for y in 0..frame.height {
        for x in 0..frame.width {
            let src = (frame.y as usize + y as usize) * chunk_width + frame.x as usize + x as usize;
            let index = *chunk.get(src)? as usize;
            if index == 0 {
                continue;
            }
            let [r, g, b] = palette[index];
            let dst =
                ((y as u32 + pad_y) as usize * width as usize + (x as u32 + pad_x) as usize) * 4;
            rgba[dst..dst + 4].copy_from_slice(&[r, g, b, 255]);
        }
    }

    Some((width, height, rgba))
}

/// Packs frames into a single atlas with per-frame uv rects.
fn pack_atlas(frames: &[(u32, u32, Vec<u8>)]) -> (WeatherSprite, Vec<SnowFrame>) {
    let mut atlas_w = 0u32;
    let mut atlas_h = 0u32;
    for (w, h, _) in frames {
        atlas_w += w;
        atlas_h = atlas_h.max(*h);
    }

    let mut pixels = vec![0u8; atlas_w as usize * atlas_h as usize * 4];
    let mut placed = Vec::with_capacity(frames.len());
    let mut cursor = 0u32;

    for (w, h, frame_pixels) in frames {
        let uv = [
            cursor as f32 / atlas_w as f32,
            0.0,
            (cursor + w) as f32 / atlas_w as f32,
            *h as f32 / atlas_h as f32,
        ];
        for y in 0..*h {
            let src = (y as usize * *w as usize) * 4;
            let dst = (y as usize * atlas_w as usize + cursor as usize) * 4;
            pixels[dst..dst + *w as usize * 4]
                .copy_from_slice(&frame_pixels[src..src + *w as usize * 4]);
        }
        placed.push(SnowFrame {
            width: *w,
            height: *h,
            uv,
        });
        cursor += w;
    }

    (
        WeatherSprite {
            width: atlas_w,
            height: atlas_h,
            pixels,
        },
        placed,
    )
}
