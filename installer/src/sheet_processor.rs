//! Builds pre-packed sprite sheets at install time.
//!
//! Each animation asset (player part, creature, item sheet, or effect) has its
//! frames shelf-packed into one or more sheet chunks. One file per asset holds
//! the oxicode metadata (frame coordinates and geometry) followed by the raw
//! chunk pixels, so the runtime can upload the pre-packed pixels straight into
//! its atlas without decoding the original format or recomputing a layout.

use formats::efa::EfaFile;
use formats::epf::{EpfAnimation, EpfImage};
use formats::mpf::MpfFile;
use formats::sheets::{
    CreatureSheet, CreatureSheetFrame, EffectSheet, EffectSheetFrame, ItemSheet,
    PlayerAnimationMeta, PlayerSheet, SheetChunk, SheetFrame,
};
use std::path::Path;

use crate::asset_record::AssetRecord;
use crate::deferred_job::EffectAssetEntries;

const PLAYER_INITIAL_SHELF_WIDTH: usize = 512;
const PLAYER_MAX_WIDTH: usize = 4096;
const PLAYER_MAX_CHUNK_HEIGHT: usize = 4096;

const CREATURE_INITIAL_SHELF_WIDTH: usize = 512;
const CREATURE_MAX_WIDTH: usize = 2048;
const CREATURE_MAX_CHUNK_HEIGHT: usize = 4096;

const ITEM_INITIAL_SHELF_WIDTH: usize = 512;
const ITEM_MAX_WIDTH: usize = 1024;
const ITEM_MAX_CHUNK_HEIGHT: usize = 1024;

const EFFECT_INITIAL_SHELF_WIDTH: usize = 512;
const EFFECT_MAX_WIDTH: usize = 2048;
const EFFECT_MAX_CHUNK_HEIGHT: usize = 2048;

pub(crate) fn build_player_sheets(animations: &[EpfAnimation]) -> (PlayerSheet, Vec<Vec<u8>>) {
    let frame_count: usize = animations.iter().map(|a| a.image.frames.len()).sum();

    let non_empty = animations
        .iter()
        .flat_map(|a| a.image.frames.iter())
        .enumerate()
        .filter_map(|(frame_index, frame)| {
            let w = (frame.right - frame.left) as usize;
            let h = (frame.bottom - frame.top) as usize;
            (w > 0 && h > 0).then_some((frame_index, w, h, frame.data.clone()))
        })
        .collect::<Vec<_>>();

    let packed = pack_frames(
        frame_count,
        non_empty,
        1,
        PLAYER_INITIAL_SHELF_WIDTH,
        PLAYER_MAX_WIDTH,
        PLAYER_MAX_CHUNK_HEIGHT,
    );

    let mut frames = vec![None; frame_count];
    let mut frame_index = 0usize;
    for anim in animations {
        for frame in &anim.image.frames {
            if let Some((chunk, x, y)) = packed.placements[frame_index] {
                frames[frame_index] = Some(SheetFrame {
                    chunk,
                    x,
                    y,
                    width: (frame.right - frame.left) as u32,
                    height: (frame.bottom - frame.top) as u32,
                    top: frame.top as i32,
                    left: frame.left as i32,
                    bottom: frame.bottom as i32,
                    right: frame.right as i32,
                });
            }
            frame_index += 1;
        }
    }

    let mut animations_meta = Vec::with_capacity(animations.len());
    let mut start_frame = 0u32;
    for anim in animations {
        let frame_count = anim.image.frames.len() as u32;
        animations_meta.push(PlayerAnimationMeta {
            animation_type: anim.animation_type,
            direction: anim.direction,
            start_frame,
            frame_count,
        });
        start_frame += frame_count;
    }

    (
        PlayerSheet {
            chunks: packed.chunks,
            frames,
            animations: animations_meta,
        },
        packed.images,
    )
}

pub(crate) fn build_creature_sheets(mpf: &MpfFile) -> (CreatureSheet, Vec<Vec<u8>>) {
    let non_empty = mpf
        .frames
        .iter()
        .enumerate()
        .filter_map(|(frame_index, frame)| {
            let w = (frame.right - frame.left) as usize;
            let h = (frame.bottom - frame.top) as usize;
            (w > 0 && h > 0).then_some((frame_index, w, h, frame.data.clone()))
        })
        .collect::<Vec<_>>();

    let packed = pack_frames(
        mpf.frames.len(),
        non_empty,
        1,
        CREATURE_INITIAL_SHELF_WIDTH,
        CREATURE_MAX_WIDTH,
        CREATURE_MAX_CHUNK_HEIGHT,
    );

    let mut frames = vec![None; mpf.frames.len()];
    for (frame_index, frame) in mpf.frames.iter().enumerate() {
        if let Some((chunk, x, y)) = packed.placements[frame_index] {
            frames[frame_index] = Some(CreatureSheetFrame {
                chunk,
                x,
                y,
                width: (frame.right - frame.left) as u32,
                height: (frame.bottom - frame.top) as u32,
                top: frame.top,
                left: frame.left,
                bottom: frame.bottom,
                right: frame.right,
                center_x: frame.center_x,
                center_y: frame.center_y,
            });
        }
    }

    (
        CreatureSheet {
            palette_number: mpf.palette_number,
            chunks: packed.chunks,
            frames,
            animations: mpf.animations.clone(),
        },
        packed.images,
    )
}

pub(crate) fn build_item_sheets(epf: &EpfImage) -> (ItemSheet, Vec<Vec<u8>>) {
    let non_empty = epf
        .frames
        .iter()
        .enumerate()
        .filter_map(|(frame_index, frame)| {
            let w = (frame.right - frame.left) as usize;
            let h = (frame.bottom - frame.top) as usize;
            (w > 0 && h > 0).then_some((frame_index, w, h, frame.data.clone()))
        })
        .collect::<Vec<_>>();

    let packed = pack_frames(
        epf.frames.len(),
        non_empty,
        1,
        ITEM_INITIAL_SHELF_WIDTH,
        ITEM_MAX_WIDTH,
        ITEM_MAX_CHUNK_HEIGHT,
    );

    let mut frames = vec![None; epf.frames.len()];
    for (frame_index, frame) in epf.frames.iter().enumerate() {
        if let Some((chunk, x, y)) = packed.placements[frame_index] {
            frames[frame_index] = Some(SheetFrame {
                chunk,
                x,
                y,
                width: (frame.right - frame.left) as u32,
                height: (frame.bottom - frame.top) as u32,
                top: frame.top as i32,
                left: frame.left as i32,
                bottom: frame.bottom as i32,
                right: frame.right as i32,
            });
        }
    }

    (
        ItemSheet {
            width: epf.width,
            height: epf.height,
            chunks: packed.chunks,
            frames,
        },
        packed.images,
    )
}

pub(crate) fn build_efa_effect_sheets(efa: &EfaFile) -> (EffectSheet, Vec<Vec<u8>>) {
    // Trim each frame to its opaque content. The stored `left`/`top` are the
    // content's origin within the image, and the frame's `center_x`/`center_y`
    // anchor is kept so the renderer can line the content up with the draw
    // point exactly as the game data intends. Fully transparent frames become
    // empty.
    let trimmed: Vec<Option<(i16, i16, i16, i16, usize, usize, Vec<u8>)>> = efa
        .frames
        .iter()
        .map(|frame| {
            let w = frame.width as usize;
            let h = frame.height as usize;
            if w == 0 || h == 0 {
                return None;
            }
            trim_rgba(&frame.data, w, h).map(|(bx0, by0, bw, bh, data)| {
                (
                    frame.left + bx0 as i16,
                    frame.top + by0 as i16,
                    frame.center_x,
                    frame.center_y,
                    bw,
                    bh,
                    data,
                )
            })
        })
        .collect();

    let non_empty = trimmed
        .iter()
        .enumerate()
        .filter_map(|(frame_index, trimmed)| {
            trimmed
                .as_ref()
                .map(|&(_, _, _, _, bw, bh, ref data)| (frame_index, bw, bh, data.clone()))
        })
        .collect::<Vec<_>>();

    let packed = pack_frames(
        efa.frames.len(),
        non_empty,
        4,
        EFFECT_INITIAL_SHELF_WIDTH,
        EFFECT_MAX_WIDTH,
        EFFECT_MAX_CHUNK_HEIGHT,
    );

    let mut frames = vec![None; efa.frames.len()];
    for (frame_index, trimmed) in trimmed.into_iter().enumerate() {
        if let (Some((chunk, x, y)), Some((left, top, center_x, center_y, bw, bh, _))) =
            (packed.placements[frame_index], trimmed)
        {
            frames[frame_index] = Some(EffectSheetFrame {
                chunk,
                x,
                y,
                width: bw as u32,
                height: bh as u32,
                left,
                top,
                center_x,
                center_y,
            });
        }
    }

    (
        EffectSheet {
            sheet_width: 0,
            sheet_height: 0,
            indexed: false,
            frame_interval_ms: efa.frame_interval_ms,
            chunks: packed.chunks,
            frames,
        },
        packed.images,
    )
}

/// Crops an RGBA frame to its opaque (alpha > 0) content, returning the crop
/// size and the cropped pixels. Pixels beyond a short buffer are treated as
/// transparent, matching how the renderer used to truncate short frames.
fn trim_rgba(
    data: &[u8],
    width: usize,
    height: usize,
) -> Option<(usize, usize, usize, usize, Vec<u8>)> {
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0usize;
    let mut max_y = 0usize;
    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) * 4;
            let alpha = if index + 3 < data.len() {
                data[index + 3]
            } else {
                0
            };
            if alpha > 0 {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x + 1);
                max_y = max_y.max(y + 1);
            }
        }
    }
    if max_x <= min_x || max_y <= min_y {
        return None;
    }

    let bw = max_x - min_x;
    let bh = max_y - min_y;
    let mut cropped = vec![0u8; bw * bh * 4];
    for y in 0..bh {
        for x in 0..bw {
            let src = ((min_y + y) * width + min_x + x) * 4;
            if src + 3 < data.len() {
                let dst = (y * bw + x) * 4;
                cropped[dst..dst + 4].copy_from_slice(&data[src..src + 4]);
            }
        }
    }
    Some((min_x, min_y, bw, bh, cropped))
}

pub(crate) fn build_epf_effect_sheets(epf: &EpfImage) -> (EffectSheet, Vec<Vec<u8>>) {
    let non_empty = epf
        .frames
        .iter()
        .enumerate()
        .filter_map(|(frame_index, frame)| {
            let w = (frame.right - frame.left) as usize;
            let h = (frame.bottom - frame.top) as usize;
            (w > 0 && h > 0).then_some((frame_index, w, h, frame.data.clone()))
        })
        .collect::<Vec<_>>();

    let packed = pack_frames(
        epf.frames.len(),
        non_empty,
        1,
        EFFECT_INITIAL_SHELF_WIDTH,
        EFFECT_MAX_WIDTH,
        EFFECT_MAX_CHUNK_HEIGHT,
    );

    let mut frames = vec![None; epf.frames.len()];
    for (frame_index, frame) in epf.frames.iter().enumerate() {
        if let Some((chunk, x, y)) = packed.placements[frame_index] {
            frames[frame_index] = Some(EffectSheetFrame {
                chunk,
                x,
                y,
                width: (frame.right - frame.left) as u32,
                height: (frame.bottom - frame.top) as u32,
                left: frame.left as i16,
                top: frame.top as i16,
                center_x: 0,
                center_y: 0,
            });
        }
    }

    (
        EffectSheet {
            sheet_width: epf.width,
            sheet_height: epf.height,
            indexed: true,
            frame_interval_ms: 100,
            chunks: packed.chunks,
            frames,
        },
        packed.images,
    )
}

/// Builds the sheet for an effect id, preferring the EFA (RGBA) variant when
/// both EFA and EPF files exist.
pub(crate) fn build_effect_sheets(
    entries: &EffectAssetEntries,
) -> Option<(EffectSheet, Vec<Vec<u8>>)> {
    if let Some(efa) = &entries.efa {
        Some(build_efa_effect_sheets(efa))
    } else if let Some(epf) = &entries.epf {
        Some(build_epf_effect_sheets(epf))
    } else {
        None
    }
}

/// Emits the single-file record for a packed sheet: `{base}.sheet.bin`
/// contains the oxicode metadata followed by the raw pixel data for every
/// chunk, concatenated in chunk order. Chunk dimensions live in the metadata,
/// so the pixel data carries no per-file header.
pub(crate) fn sheet_records(
    dat_path: &Path,
    base: &str,
    bytes_per_pixel: u32,
    sheet_bytes: Vec<u8>,
    chunks: Vec<SheetChunk>,
    images: Vec<Vec<u8>>,
) -> anyhow::Result<Vec<AssetRecord>> {
    anyhow::ensure!(
        images.len() == chunks.len(),
        "sheet has {} chunks but {} images",
        chunks.len(),
        images.len()
    );
    let mut file_bytes = sheet_bytes;
    for (chunk, image) in chunks.iter().zip(&images) {
        let expected = chunk.width as usize * chunk.height as usize * bytes_per_pixel as usize;
        anyhow::ensure!(
            image.len() == expected,
            "chunk pixel data size mismatch: expected {expected} bytes, got {}",
            image.len()
        );
        file_bytes.extend_from_slice(image);
    }
    Ok(vec![AssetRecord::bytes(
        dat_path.join(format!("{base}.sheet.bin")),
        file_bytes,
    )])
}

struct PackedFrames {
    chunks: Vec<SheetChunk>,
    /// `(chunk, x, y)` per frame, in file order; `None` for empty frames.
    placements: Vec<Option<(u32, u32, u32)>>,
    /// One row-major pixel buffer per chunk.
    images: Vec<Vec<u8>>,
}

fn pack_frames(
    frame_count: usize,
    non_empty: Vec<(usize, usize, usize, Vec<u8>)>,
    bytes_per_pixel: usize,
    initial_target_width: usize,
    max_width: usize,
    max_chunk_height: usize,
) -> PackedFrames {
    if non_empty.is_empty() {
        return PackedFrames {
            chunks: Vec::new(),
            placements: vec![None; frame_count],
            images: Vec::new(),
        };
    }

    let sizes = non_empty
        .iter()
        .map(|&(frame_index, w, h, _)| (frame_index, w, h))
        .collect::<Vec<_>>();
    let chunks =
        formats::sheets::pack_chunks(&sizes, initial_target_width, max_width, max_chunk_height);

    let mut placements = vec![None; frame_count];
    let mut images = Vec::with_capacity(chunks.len());
    let mut chunk_meta = Vec::with_capacity(chunks.len());

    for (chunk_index, (placed, slot_width, slot_height)) in chunks.into_iter().enumerate() {
        let mut image = vec![0u8; slot_width * slot_height * bytes_per_pixel];
        for (frame_index, x, y) in placed {
            let (_, w, h, data) = non_empty
                .iter()
                .find(|entry| entry.0 == frame_index)
                .expect("packed frame must have pixel data");
            placements[frame_index] = Some((chunk_index as u32, x as u32, y as u32));
            for row in 0..*h {
                let row_bytes = w * bytes_per_pixel;
                let start = row * row_bytes;
                let Some(end) = start.checked_add(row_bytes) else {
                    break;
                };
                if end > data.len() {
                    // Some EFA frames carry fewer bytes than their declared
                    // size. Copy only the complete rows; the rest stays zero
                    // (transparent), matching the previous runtime uploader
                    // which skipped trailing partial rows.
                    tracing::warn!(
                        frame_index,
                        expected = w * h * bytes_per_pixel,
                        actual = data.len(),
                        "Frame pixel data is shorter than its declared size; truncated"
                    );
                    break;
                }
                let src = &data[start..end];
                let dst_start = ((y + row) * slot_width + x) * bytes_per_pixel;
                let dst = &mut image[dst_start..dst_start + row_bytes];
                dst.copy_from_slice(src);
            }
        }
        chunk_meta.push(SheetChunk {
            width: slot_width as u32,
            height: slot_height as u32,
        });
        images.push(image);
    }

    PackedFrames {
        chunks: chunk_meta,
        placements,
        images,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn epf_animation(frames: usize) -> EpfAnimation {
        EpfAnimation {
            animation_type: formats::epf::EpfAnimationType::Idle,
            direction: formats::epf::AnimationDirection::Away,
            image: EpfImage {
                width: 64,
                height: 64,
                frames: (0..frames)
                    .map(|i| {
                        formats::epf::EpfFrame::new(
                            0,
                            i as u16,
                            i as u16 + 32,
                            16,
                            vec![7; 16 * 32],
                        )
                    })
                    .collect(),
            },
        }
    }

    #[test]
    fn player_sheets_round_trip_frame_geometry_and_placements() {
        let animations = vec![epf_animation(4), epf_animation(2)];
        let (sheet, images) = build_player_sheets(&animations);

        assert_eq!(sheet.animations.len(), 2);
        assert_eq!(sheet.animations[0].start_frame, 0);
        assert_eq!(sheet.animations[0].frame_count, 4);
        assert_eq!(sheet.animations[1].start_frame, 4);
        assert_eq!(sheet.animations[1].frame_count, 2);
        assert_eq!(sheet.frames.len(), 6);
        assert!(sheet.frames.iter().all(|f| f.is_some()));

        let chunk = sheet.chunks[0];
        assert_eq!(images.len(), sheet.chunks.len());
        assert_eq!(
            images[0].len(),
            chunk.width as usize * chunk.height as usize
        );

        for frame in sheet.frames.iter().flatten() {
            assert!(frame.width >= 1);
            assert!(frame.height >= 1);
            assert_eq!(frame.width as i32, frame.right - frame.left);
            assert_eq!(frame.height as i32, frame.bottom - frame.top);
        }
    }

    #[test]
    fn creature_sheets_preserve_palette_and_animations() {
        let mpf = MpfFile {
            palette_number: 12,
            width: 64,
            height: 64,
            animations: vec![formats::mpf::MpfAnimation::new(
                formats::mpf::MpfAnimationType::Standing,
                0,
                2,
                true,
            )],
            frames: vec![
                formats::mpf::MpfFrame {
                    top: 0,
                    left: 0,
                    bottom: 32,
                    right: 16,
                    center_x: 8,
                    center_y: 16,
                    data: vec![3; 16 * 32],
                },
                formats::mpf::MpfFrame {
                    top: 0,
                    left: 0,
                    bottom: 0,
                    right: 0,
                    center_x: 0,
                    center_y: 0,
                    data: vec![],
                },
            ],
        };

        let (sheet, images) = build_creature_sheets(&mpf);
        assert_eq!(sheet.palette_number, 12);
        assert_eq!(sheet.animations.len(), 1);
        assert!(sheet.frames[0].is_some());
        assert!(sheet.frames[1].is_none());
        assert_eq!(images.len(), 1);
    }

    #[test]
    fn item_sheets_keep_epf_canvas_size() {
        let epf = EpfImage {
            width: 40,
            height: 40,
            frames: vec![formats::epf::EpfFrame::new(0, 0, 16, 16, vec![1; 256])],
        };
        let (sheet, images) = build_item_sheets(&epf);
        assert_eq!((sheet.width, sheet.height), (40, 40));
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].len(), 16 * 16);
    }

    #[test]
    fn effect_sheets_flag_indexed_pixels() {
        let efa = EfaFile {
            frame_interval_ms: 42,
            frames: vec![formats::efa::EfaFrame {
                width: 8,
                height: 8,
                left: -4,
                top: -4,
                center_x: 4,
                center_y: 8,
                data: vec![255; 8 * 8 * 4],
            }],
        };
        let (efa_sheet, efa_images) = build_efa_effect_sheets(&efa);
        assert!(!efa_sheet.indexed);
        assert_eq!(efa_sheet.frame_interval_ms, 42);
        assert_eq!(efa_images[0].len(), 8 * 8 * 4);
        assert_eq!(efa_sheet.frames[0].unwrap().left, -4);

        let epf = EpfImage {
            width: 64,
            height: 64,
            frames: vec![formats::epf::EpfFrame::new(0, 0, 32, 32, vec![2; 1024])],
        };
        let (epf_sheet, epf_images) = build_epf_effect_sheets(&epf);
        assert!(epf_sheet.indexed);
        assert_eq!((epf_sheet.sheet_width, epf_sheet.sheet_height), (64, 64));
        assert_eq!(epf_images[0].len(), 32 * 32);
    }

    #[test]
    fn all_empty_frames_produce_no_chunks() {
        let epf = EpfImage {
            width: 32,
            height: 32,
            frames: vec![
                formats::epf::EpfFrame::new(0, 0, 0, 0, vec![]),
                formats::epf::EpfFrame::new(0, 0, 0, 0, vec![]),
            ],
        };
        let (sheet, images) = build_item_sheets(&epf);
        assert!(sheet.chunks.is_empty());
        assert!(images.is_empty());
        assert_eq!(sheet.frames.len(), 2);
        assert!(sheet.frames.iter().all(|f| f.is_none()));
    }

    #[test]
    fn effect_entries_prefer_efa_over_epf() {
        let mut entries = crate::deferred_job::EffectAssetEntries::default();
        entries.epf = Some(EpfImage {
            width: 32,
            height: 32,
            frames: vec![formats::epf::EpfFrame::new(0, 0, 16, 16, vec![1; 256])],
        });
        assert!(build_effect_sheets(&entries).unwrap().0.indexed);

        entries.efa = Some(EfaFile {
            frame_interval_ms: 25,
            frames: vec![formats::efa::EfaFrame {
                width: 8,
                height: 8,
                left: 0,
                top: 0,
                center_x: 4,
                center_y: 8,
                data: vec![255; 8 * 8 * 4],
            }],
        });
        let (sheet, _) = build_effect_sheets(&entries).expect("sheet built");
        assert!(!sheet.indexed);
        assert_eq!(sheet.frame_interval_ms, 25);
    }

    #[test]
    fn packed_images_place_pixels_at_recorded_coordinates() {
        // Two frames with distinct pixel values so placement mistakes are
        // detectable in the resulting sheet image.
        let epf = EpfImage {
            width: 64,
            height: 64,
            frames: vec![
                formats::epf::EpfFrame::new(0, 0, 16, 16, vec![1; 16 * 16]),
                formats::epf::EpfFrame::new(0, 0, 8, 24, vec![2; 8 * 24]),
            ],
        };
        let (sheet, images) = build_item_sheets(&epf);
        assert_eq!(images.len(), 1);

        for (frame_index, frame) in sheet.frames.iter().enumerate() {
            let frame = frame.expect("non-empty frame");
            let image = &images[frame.chunk as usize];
            let chunk = sheet.chunks[frame.chunk as usize];
            let expected = frame_index as u8 + 1;
            for row in 0..frame.height {
                for col in 0..frame.width {
                    let pixel = image[((frame.y + row) * chunk.width + frame.x + col) as usize];
                    assert_eq!(pixel, expected);
                }
            }
        }
    }

    #[test]
    fn short_efa_frame_data_is_trimmed_to_available_content() {
        // Some EFA frames carry fewer bytes than width * height * 4. Trimming
        // crops to the opaque content, so the sheet is fully populated and no
        // partial rows are packed.
        let efa = EfaFile {
            frame_interval_ms: 100,
            frames: vec![formats::efa::EfaFrame {
                width: 12,
                height: 8,
                left: 0,
                top: 0,
                center_x: 6,
                center_y: 8,
                data: vec![128; 12 * 6 * 4], // rows 0..6 only
            }],
        };
        let (sheet, images) = build_efa_effect_sheets(&efa);
        let frame = sheet.frames[0].expect("frame packed");
        assert_eq!(frame.width, 12);
        assert_eq!(frame.height, 6);

        let image = &images[frame.chunk as usize];
        let chunk = sheet.chunks[frame.chunk as usize];
        // Every trimmed row holds source pixels; nothing is left transparent.
        for row in 0..frame.height {
            for col in 0..12 {
                let offset = (((frame.y + row) * chunk.width + frame.x + col) * 4) as usize;
                assert_eq!(image[offset], 128);
            }
        }
    }

    #[test]
    fn efa_frames_trim_transparent_padding_and_keep_offsets() {
        // 10x10 frame with opaque content only in the 4x4 region at (3,2).
        let mut data = vec![0u8; 10 * 10 * 4];
        for y in 2..6 {
            for x in 3..7 {
                let index = (y * 10 + x) * 4;
                data[index..index + 4].copy_from_slice(&[200, 100, 50, 255]);
            }
        }
        let efa = EfaFile {
            frame_interval_ms: 50,
            frames: vec![formats::efa::EfaFrame {
                width: 10,
                height: 10,
                left: -5,
                top: -8,
                center_x: 5,
                center_y: 10,
                data,
            }],
        };

        let (sheet, images) = build_efa_effect_sheets(&efa);
        let frame = sheet.frames[0].expect("frame packed");
        // The stored offsets are the content's origin within the image, and
        // the frame's anchor is carried through for placement.
        assert_eq!((frame.left, frame.top), (-2, -6));
        assert_eq!((frame.center_x, frame.center_y), (5, 10));
        assert_eq!((frame.width, frame.height), (4, 4));

        let image = &images[frame.chunk as usize];
        assert_eq!(image.len(), 4 * 4 * 4);
        // First trimmed pixel is the original frame's (3,2) pixel.
        assert_eq!(image[..4], [200, 100, 50, 255]);
    }
}
