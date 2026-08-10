//! Pre-packed sprite sheets.
//!
//! The installer packs every animation asset (player part, creature, item
//! sheet, or effect) into one or more sheet chunks and writes a single file
//! per asset: oxicode metadata followed by the raw pixel data for every
//! chunk. At runtime the scene stores decode the metadata, allocate atlas
//! slots for the chunks, and upload the pre-packed pixels - they never decode
//! the original animation format or compute a frame layout themselves.

use crate::epf::{AnimationDirection, EpfAnimationType};
use crate::mpf::MpfAnimation;
use oxicode::{Decode, Encode};

/// A placed frame: `(frame_index, x, y)`.
type PlacedFrame = (usize, usize, usize);
/// One packed chunk: placed frames plus the chunk's pixel dimensions.
type PackedChunk = (Vec<PlacedFrame>, usize, usize);

/// One image in a packed sheet. Most assets have exactly one chunk; very tall
/// stacks of frames (rare) are split into several chunks so each stays
/// packable inside the runtime atlas.
#[derive(Clone, Copy, Debug, Decode, Encode, PartialEq, Eq)]
pub struct SheetChunk {
    pub width: u32,
    pub height: u32,
}

/// A frame's placement inside a sheet chunk plus the frame geometry the
/// renderer still needs to position and size the quad.
#[derive(Clone, Copy, Debug, Decode, Encode, PartialEq, Eq)]
pub struct SheetFrame {
    /// Index into the sheet's `chunks`; the frame lives in that sheet image.
    pub chunk: u32,
    /// Offset of the frame's pixels inside the chunk image.
    pub x: u32,
    pub y: u32,
    /// Frame pixel dimensions (`right - left`, `bottom - top`).
    pub width: u32,
    pub height: u32,
    pub top: i32,
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
}

/// Metadata for one player part (`khan/...`), written as `{base}.sheet.bin`.
#[derive(Clone, Debug, Decode, Encode, PartialEq)]
pub struct PlayerSheet {
    pub chunks: Vec<SheetChunk>,
    /// One entry per animation frame, in file order. `None` for empty frames,
    /// which are never rendered.
    pub frames: Vec<Option<SheetFrame>>,
    pub animations: Vec<PlayerAnimationMeta>,
}

#[derive(Clone, Copy, Debug, Decode, Encode, PartialEq, Eq)]
pub struct PlayerAnimationMeta {
    pub animation_type: EpfAnimationType,
    pub direction: AnimationDirection,
    /// Index of the animation's first frame in `PlayerSheet::frames`.
    pub start_frame: u32,
    pub frame_count: u32,
}

/// Metadata for one creature (`hades/mnsNNN`), written as `{base}.sheet.bin`.
#[derive(Clone, Debug, Decode, Encode)]
pub struct CreatureSheet {
    pub palette_number: u8,
    pub chunks: Vec<SheetChunk>,
    pub frames: Vec<Option<CreatureSheetFrame>>,
    pub animations: Vec<MpfAnimation>,
}

#[derive(Clone, Copy, Debug, Decode, Encode, PartialEq, Eq)]
pub struct CreatureSheetFrame {
    pub chunk: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub top: i16,
    pub left: i16,
    pub bottom: i16,
    pub right: i16,
    pub center_x: i16,
    pub center_y: i16,
}

/// Metadata for one item sheet (`Legend/itemNNN`), written as `{base}.sheet.bin`.
#[derive(Clone, Debug, Decode, Encode, PartialEq, Eq)]
pub struct ItemSheet {
    /// EPF canvas size used for item placement.
    pub width: u16,
    pub height: u16,
    pub chunks: Vec<SheetChunk>,
    pub frames: Vec<Option<SheetFrame>>,
}

/// Metadata for one effect (`roh/efctNNN`), written as `{base}.sheet.bin`.
#[derive(Clone, Debug, Decode, Encode, PartialEq, Eq)]
pub struct EffectSheet {
    /// 0 for EFA effects (which use direct frame offsets); the EPF canvas size
    /// for EPF effects.
    pub sheet_width: u16,
    pub sheet_height: u16,
    /// True when the sheet pixels are palette indices that must be baked to
    /// RGBA before upload (EPF effects); false for already-RGBA EFA effects.
    pub indexed: bool,
    pub frame_interval_ms: usize,
    pub chunks: Vec<SheetChunk>,
    pub frames: Vec<Option<EffectSheetFrame>>,
}

#[derive(Clone, Copy, Debug, Decode, Encode, PartialEq, Eq)]
pub struct EffectSheetFrame {
    pub chunk: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub left: i16,
    pub top: i16,
    /// EFA anchor point (image coords) that lines up with the draw point.
    /// Zero for EPF effects.
    pub center_x: i16,
    pub center_y: i16,
}

/// A sheet metadata type that knows its chunks (and therefore its pixel
/// layout).
pub trait SheetMeta {
    fn chunks(&self) -> &[SheetChunk];
}

impl SheetMeta for PlayerSheet {
    fn chunks(&self) -> &[SheetChunk] {
        &self.chunks
    }
}

impl SheetMeta for CreatureSheet {
    fn chunks(&self) -> &[SheetChunk] {
        &self.chunks
    }
}

impl SheetMeta for ItemSheet {
    fn chunks(&self) -> &[SheetChunk] {
        &self.chunks
    }
}

impl SheetMeta for EffectSheet {
    fn chunks(&self) -> &[SheetChunk] {
        &self.chunks
    }
}

/// Byte offset of each chunk's pixel data inside a sheet file's trailing
/// pixel blob, plus the total blob length. Rows are tight (`width * height`
/// pixels per chunk, `bytes_per_pixel` bytes per pixel).
pub fn chunk_pixel_offsets(
    chunks: &[SheetChunk],
    bytes_per_pixel: u32,
) -> (Vec<usize>, usize) {
    let mut offsets = Vec::with_capacity(chunks.len());
    let mut total = 0usize;
    for chunk in chunks {
        offsets.push(total);
        total += chunk.width as usize * chunk.height as usize * bytes_per_pixel as usize;
    }
    (offsets, total)
}

/// Encodes one sheet file: oxicode metadata followed by the raw pixel data
/// for every chunk, concatenated in chunk order. Chunk dimensions live in the
/// metadata, so the pixel data carries no per-file header.
pub fn encode_sheet<M: Encode>(meta: &M, images: Vec<Vec<u8>>) -> anyhow::Result<Vec<u8>> {
    let mut bytes = oxicode::encode_to_vec(meta)?;
    for image in images {
        bytes.extend_from_slice(&image);
    }
    Ok(bytes)
}

/// Borrows each chunk's pixel data out of a sheet file's trailing pixel blob,
/// which starts `consumed` bytes after the oxicode metadata. Returns an error
/// if the blob is not exactly the size the chunk dimensions imply. The
/// returned slices borrow from `bytes`, so `bytes` must outlive them.
pub fn chunk_pixel_slices<'a>(
    bytes: &'a [u8],
    consumed: usize,
    chunks: &[SheetChunk],
    bytes_per_pixel: u32,
) -> anyhow::Result<Vec<&'a [u8]>> {
    let (offsets, total) = chunk_pixel_offsets(chunks, bytes_per_pixel);
    let pixels = bytes
        .get(consumed..)
        .ok_or_else(|| anyhow::anyhow!("sheet file missing pixel data"))?;
    anyhow::ensure!(
        pixels.len() == total,
        "sheet pixel data size mismatch: expected {total} bytes, got {}",
        pixels.len()
    );

    let mut slices = Vec::with_capacity(chunks.len());
    for (index, chunk) in chunks.iter().enumerate() {
        let len = chunk.width as usize * chunk.height as usize * bytes_per_pixel as usize;
        let start = offsets[index];
        slices.push(&pixels[start..start + len]);
    }
    Ok(slices)
}

/// Copies each chunk's pixel data out of a sheet file's trailing pixel blob,
/// which starts `consumed` bytes after the oxicode metadata.
pub fn chunk_pixels(
    bytes: &[u8],
    consumed: usize,
    chunks: &[SheetChunk],
    bytes_per_pixel: u32,
) -> anyhow::Result<Vec<Vec<u8>>> {
    Ok(chunk_pixel_slices(bytes, consumed, chunks, bytes_per_pixel)?
        .into_iter()
        .map(|slice| slice.to_vec())
        .collect())
}

/// Decodes one sheet file into its metadata and one owned pixel buffer per
/// chunk (copies the pixel blob).
pub fn decode_sheet<M: Decode + SheetMeta>(
    bytes: &[u8],
    bytes_per_pixel: u32,
) -> anyhow::Result<(M, Vec<Vec<u8>>)> {
    let (meta, consumed): (M, usize) = oxicode::decode_from_slice(bytes)?;
    let slices = chunk_pixel_slices(bytes, consumed, meta.chunks(), bytes_per_pixel)?;
    Ok((
        meta,
        slices.into_iter().map(|slice| slice.to_vec()).collect(),
    ))
}

/// Shelf-packs `(frame_index, width, height)` entries into rows of at most
/// `target_width`, returning the placed positions plus the packed rectangle
/// dimensions. Keeps slots short and wide so atlas packers can place them
/// efficiently.
pub fn shelf_layout(
    frames: &[(usize, usize, usize)],
    target_width: usize,
) -> (Vec<(usize, usize, usize)>, usize, usize) {
    let mut placed = Vec::with_capacity(frames.len());
    let mut slot_width = 0usize;
    let mut shelf_y = 0usize;
    let mut shelf_h = 0usize;
    let mut row_x = 0usize;
    for &(frame_index, w, h) in frames {
        if row_x > 0 && row_x + w > target_width {
            shelf_y += shelf_h;
            row_x = 0;
            shelf_h = 0;
        }
        placed.push((frame_index, row_x, shelf_y));
        row_x += w;
        slot_width = slot_width.max(row_x);
        shelf_h = shelf_h.max(h);
    }
    (placed, slot_width, shelf_y + shelf_h)
}

/// Packs `(frame_index, width, height)` entries into one or more chunks.
///
/// Frames are sorted by height and placed with a maxrects bin packer (best
/// short-side fit). Several bin widths from `initial_target_width` up to
/// `max_width` are tried and the layout with the smallest packed area wins,
/// keeping the runtime atlas footprint (and upload size) as small as possible.
/// Layouts that fit within `max_chunk_height` are preferred over chunked
/// ones. Stacks that still do not fit are split into chunks along horizontal
/// bands so every chunk stays at most `max_chunk_height` tall.
pub fn pack_chunks(
    frames: &[PlacedFrame],
    initial_target_width: usize,
    max_width: usize,
    max_chunk_height: usize,
) -> Vec<PackedChunk> {
    if frames.is_empty() {
        return Vec::new();
    }

    // Tall frames first so rows/pieces of similar height stay together and
    // small frames can fill the gaps below them.
    let mut sorted = frames.to_vec();
    sorted.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.cmp(&a.1)));

    let max_frame_width = sorted.iter().map(|&(_, w, _)| w).max().unwrap_or(0);
    let mut width = initial_target_width.max(max_frame_width);
    // (bin width, placements, content width, height) of the best layout so far.
    let mut best: Option<(usize, Vec<PlacedFrame>, usize, usize)> = None;

    loop {
        if let Some((placed, height)) = maxrects_pack(&sorted, width) {
            // The bin may be wider than the content; report the content width
            // so chunk images and atlas slots don't carry useless padding.
            let content_width = placed
                .iter()
                .map(|&(index, x, _)| {
                    let w = sorted
                        .iter()
                        .find(|&&(fi, _, _)| fi == index)
                        .map(|&(_, w, _)| w)
                        .unwrap_or(0);
                    x + w
                })
                .max()
                .unwrap_or(0);
            let better = match &best {
                None => true,
                Some(best_layout) => {
                    let (best_bin_width, _, best_content, best_height) = best_layout;
                    let fits = height <= max_chunk_height;
                    let best_fits = *best_height <= max_chunk_height;
                    match (fits, best_fits) {
                        (true, false) => true,
                        (false, true) => false,
                        _ => {
                            let area = content_width * height;
                            let best_area = *best_content * *best_height;
                            area < best_area || (area == best_area && width < *best_bin_width)
                        }
                    }
                }
            };
            if better {
                best = Some((width, placed, content_width, height));
            }
        }
        if width >= max_width {
            break;
        }
        let next = (width * 2).min(max_width).max(max_frame_width);
        if next <= width {
            break;
        }
        width = next;
    }

    let (_, placed, content_width, height) = best.unwrap_or_else(|| {
        // Fallback: height-sorted shelf packing always places every frame.
        let (placed, width, height) = shelf_layout(&sorted, width);
        (width, placed, width, height)
    });
    split_chunks(&sorted, &placed, content_width, height, max_chunk_height)
}

/// Places `frames` (assumed height-sorted) into a bin of `bin_width` using
/// maxrects with best short-side fit. Returns per-frame `(index, x, y)`
/// placements and the bin height used, or `None` if a frame could not be
/// placed (bin too narrow for the set).
fn maxrects_pack(frames: &[PlacedFrame], bin_width: usize) -> Option<(Vec<PlacedFrame>, usize)> {
    #[derive(Clone, Copy, Debug)]
    struct FreeRect {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    }

    let mut free = vec![FreeRect {
        x: 0,
        y: 0,
        w: bin_width as i32,
        h: i32::MAX,
    }];
    let mut placed = Vec::with_capacity(frames.len());
    let mut bin_height = 0usize;

    for &(index, w, h) in frames {
        let (w, h) = (w as i32, h as i32);
        let mut best_score = (i32::MAX, i32::MAX);
        let mut best_index = None;
        for (i, rect) in free.iter().enumerate() {
            if rect.w >= w && rect.h >= h {
                let score = ((rect.w - w).min(rect.h - h), (rect.w - w).max(rect.h - h));
                if score < best_score {
                    best_score = score;
                    best_index = Some(i);
                }
            }
        }
        let rect_index = best_index?;
        let rect = free.remove(rect_index);
        placed.push((index, rect.x as usize, rect.y as usize));
        bin_height = bin_height.max(rect.y as usize + h as usize);

        // Split the leftover around the placed rect; prune any free rectangle
        // now contained inside another so the free list stays maximal.
        let add_free_rect = |free: &mut Vec<FreeRect>, candidate: FreeRect| {
            if candidate.w <= 0 || candidate.h <= 0 {
                return;
            }
            // Drop existing free rects fully contained in the candidate.
            free.retain(|existing| {
                !(candidate.x <= existing.x
                    && candidate.y <= existing.y
                    && candidate.x + candidate.w >= existing.x + existing.w
                    && candidate.y + candidate.h >= existing.y + existing.h)
            });
            // Skip the candidate if an existing free rect already covers it.
            if free.iter().any(|existing| {
                existing.x <= candidate.x
                    && existing.y <= candidate.y
                    && existing.x + existing.w >= candidate.x + candidate.w
                    && existing.y + existing.h >= candidate.y + candidate.h
            }) {
                return;
            }
            free.push(candidate);
        };
        if rect.w > w {
            add_free_rect(
                &mut free,
                FreeRect {
                    x: rect.x + w,
                    y: rect.y,
                    w: rect.w - w,
                    h,
                },
            );
        }
        if rect.h > h {
            add_free_rect(
                &mut free,
                FreeRect {
                    x: rect.x,
                    y: rect.y + h,
                    w: rect.w,
                    h: rect.h - h,
                },
            );
        }
    }

    Some((placed, bin_height))
}

/// Splits absolute placements into chunks of at most `max_chunk_height` along
/// horizontal bands. A frame lives entirely in the chunk where it starts; the
/// chunk image grows to hold frames that cross a band boundary.
fn split_chunks(
    frames: &[PlacedFrame],
    placed: &[PlacedFrame],
    width: usize,
    height: usize,
    max_chunk_height: usize,
) -> Vec<PackedChunk> {
    if height <= max_chunk_height {
        return vec![(placed.to_vec(), width, height)];
    }

    let frame_height = |index: usize| {
        frames
            .iter()
            .find(|&&(fi, _, _)| fi == index)
            .map(|&(_, _, h)| h)
            .unwrap_or(0)
    };

    let mut chunks = Vec::new();
    let mut band_top = 0usize;
    while band_top < height {
        let band_bottom = (band_top + max_chunk_height).min(height);
        let mut chunk_placed = Vec::new();
        let mut chunk_height = 0usize;
        for &(index, x, y) in placed {
            if y >= band_top && y < band_bottom {
                let shifted_y = y - band_top;
                chunk_placed.push((index, x, shifted_y));
                chunk_height = chunk_height.max(shifted_y + frame_height(index));
            }
        }
        if !chunk_placed.is_empty() {
            chunks.push((chunk_placed, width, chunk_height));
        }
        band_top = band_bottom;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::{
        CreatureSheet, CreatureSheetFrame, EffectSheet, EffectSheetFrame, ItemSheet, PackedChunk,
        PlacedFrame, PlayerSheet, SheetChunk, SheetFrame, decode_sheet, encode_sheet, pack_chunks,
        shelf_layout,
    };
    use crate::epf::{AnimationDirection, EpfAnimationType};
    use crate::mpf::{MpfAnimation, MpfAnimationType};

    #[test]
    fn shelf_layout_wraps_rows_at_target_width() {
        let frames = vec![(0, 100, 50), (1, 100, 50), (2, 100, 50), (3, 100, 50)];
        let (placed, width, height) = shelf_layout(&frames, 250);
        assert_eq!(width, 200);
        assert_eq!(height, 100);
        assert_eq!(placed[0], (0, 0, 0));
        assert_eq!(placed[1], (1, 100, 0));
        assert_eq!(placed[2], (2, 0, 50));
        assert_eq!(placed[3], (3, 100, 50));
    }

    #[test]
    fn sheet_file_round_trips_metadata_and_raw_pixels() {
        let chunks = vec![
            SheetChunk {
                width: 4,
                height: 2,
            },
            SheetChunk {
                width: 2,
                height: 2,
            },
        ];
        let images = vec![vec![1u8; 4 * 2], vec![2u8; 2 * 2]];
        let meta = ItemSheet {
            width: 4,
            height: 2,
            chunks: chunks.clone(),
            frames: vec![Some(SheetFrame {
                chunk: 0,
                x: 0,
                y: 0,
                width: 2,
                height: 2,
                top: 0,
                left: 0,
                bottom: 2,
                right: 2,
            })],
        };

        let file = encode_sheet(&meta, images.clone()).unwrap();
        let (decoded, pixels) = decode_sheet::<ItemSheet>(&file, 1).unwrap();
        assert_eq!(decoded, meta);
        assert_eq!(pixels, images);
    }

    #[test]
    fn decode_sheet_rejects_truncated_pixel_data() {
        let meta = ItemSheet {
            width: 4,
            height: 2,
            chunks: vec![SheetChunk {
                width: 4,
                height: 2,
            }],
            frames: Vec::new(),
        };
        let mut file = encode_sheet(&meta, vec![vec![1u8; 8]]).unwrap();
        file.pop(); // truncate one pixel
        assert!(decode_sheet::<ItemSheet>(&file, 1).is_err());
    }

    #[test]
    fn pack_chunks_splits_tall_stacks() {
        // Thirty 100x400 frames stack three rows high even at a 1024-wide
        // shelf, so the packer splits them across shelf-row boundaries.
        let frames = (0..30).map(|i| (i, 100, 400)).collect::<Vec<_>>();
        let chunks = pack_chunks(&frames, 512, 1024, 500);
        assert!(chunks.len() >= 2);

        let mut seen = std::collections::HashSet::new();
        for (placed, width, height) in &chunks {
            for &(frame_index, x, y) in placed {
                assert!(seen.insert(frame_index));
                let (_, w, h) = frames[frame_index];
                assert!(x + w <= *width);
                assert!(y + h <= *height);
            }
        }
        assert_eq!(seen.len(), frames.len());
    }

    #[test]
    fn pack_chunks_keeps_single_chunk_when_short() {
        let frames = (0..3).map(|i| (i, 100, 100)).collect::<Vec<_>>();
        let chunks = pack_chunks(&frames, 512, 1024, 1000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].1, 300);
        assert_eq!(chunks[0].2, 100);
    }

    #[test]
    fn maxrects_fills_gaps_better_than_shelf() {
        // A 60x60 frame leaves a 40-wide gap beside it; the packer tucks the
        // 40x40 and 20x20 frames into it instead of stacking rows, and the
        // width search finds the tightest layout (140x60 here) rather than
        // blindly using the narrowest bin (100x100).
        let frames = vec![
            (0, 60, 60),
            (1, 40, 40),
            (2, 40, 40),
            (3, 20, 20),
            (4, 20, 20),
        ];
        let chunks = pack_chunks(&frames, 100, 400, 1000);
        assert_eq!(chunks.len(), 1);
        let (_, width, height) = chunks[0];
        // A height-sorted shelf in a 100-wide bin would need 100x120.
        assert!(
            width * height < 100 * 120,
            "expected tighter than shelf, got {width}x{height}"
        );
        assert_no_overlaps(&frames, &chunks);
    }

    #[test]
    fn maxrects_never_overlaps_or_loses_frames() {
        let frames = vec![
            (0, 37, 61),
            (1, 12, 48),
            (2, 91, 22),
            (3, 25, 73),
            (4, 60, 60),
            (5, 44, 31),
            (6, 18, 18),
            (7, 7, 7),
            (8, 33, 55),
            (9, 50, 24),
        ];
        let chunks = pack_chunks(&frames, 128, 256, 1000);
        assert_eq!(chunks.len(), 1);
        assert_no_overlaps(&frames, &chunks);

        let mut seen = std::collections::HashSet::new();
        for (placed, width, height) in &chunks {
            for &(index, x, y) in placed {
                assert!(seen.insert(index));
                let (_, w, h) = frames[index];
                assert!(x + w <= *width);
                assert!(y + h <= *height);
            }
        }
        assert_eq!(seen.len(), frames.len());
    }

    #[test]
    fn packing_speed_smoke() {
        let mut frames = Vec::new();
        for i in 0..266usize {
            let w = 20 + (i * 7) % 50;
            let h = 20 + (i * 13) % 80;
            frames.push((i, w, h));
        }
        let start = std::time::Instant::now();
        let chunks = pack_chunks(&frames, 512, 1024, 1024);
        let elapsed = start.elapsed();
        eprintln!(
            "266 mixed frames -> {} chunks in {:?}",
            chunks.len(),
            elapsed
        );

        let mut frames = Vec::new();
        for i in 0..300usize {
            let w = 30 + (i * 5) % 40;
            let h = 40 + (i * 11) % 70;
            frames.push((i, w, h));
        }
        let start = std::time::Instant::now();
        let chunks = pack_chunks(&frames, 512, 4096, 4096);
        let elapsed = start.elapsed();
        eprintln!(
            "300 player frames -> {} chunks in {:?}",
            chunks.len(),
            elapsed
        );

        let mut frames = Vec::new();
        for i in 0..800usize {
            let w = 5 + ((i * 37) % 400);
            let h = 5 + ((i * 53) % 300);
            frames.push((i, w, h));
        }
        let start = std::time::Instant::now();
        let chunks = pack_chunks(&frames, 512, 2048, 2048);
        let elapsed = start.elapsed();
        eprintln!(
            "800 wildly varied frames -> {} chunks in {:?}",
            chunks.len(),
            elapsed
        );
    }

    fn assert_no_overlaps(frames: &[PlacedFrame], chunks: &[PackedChunk]) {
        for (placed, _, _) in chunks {
            for (i, &(a, ax, ay)) in placed.iter().enumerate() {
                let (_, aw, ah) = frames[a];
                for &(b, bx, by) in &placed[i + 1..] {
                    let (_, bw, bh) = frames[b];
                    let overlaps = ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah;
                    assert!(!overlaps, "frames {a} and {b} overlap");
                }
            }
        }
    }

    #[test]
    fn sheet_metadata_round_trips_through_oxicode() {
        let chunk = SheetChunk {
            width: 128,
            height: 64,
        };
        let frame = SheetFrame {
            chunk: 0,
            x: 16,
            y: 8,
            width: 32,
            height: 48,
            top: 2,
            left: 4,
            bottom: 50,
            right: 36,
        };

        let player = PlayerSheet {
            chunks: vec![chunk],
            frames: vec![Some(frame), None],
            animations: vec![super::PlayerAnimationMeta {
                animation_type: EpfAnimationType::Idle,
                direction: AnimationDirection::Towards,
                start_frame: 0,
                frame_count: 2,
            }],
        };
        let decoded: PlayerSheet =
            oxicode::decode_from_slice(&oxicode::encode_to_vec(&player).unwrap())
                .unwrap()
                .0;
        assert_eq!(decoded, player);

        let creature = CreatureSheet {
            palette_number: 7,
            chunks: vec![chunk],
            frames: vec![Some(CreatureSheetFrame {
                chunk: 0,
                x: 1,
                y: 2,
                width: 3,
                height: 4,
                top: 5,
                left: 6,
                bottom: 9,
                right: 9,
                center_x: 7,
                center_y: 8,
            })],
            animations: vec![MpfAnimation::new(MpfAnimationType::Standing, 0, 1, false)],
        };
        let decoded: CreatureSheet =
            oxicode::decode_from_slice(&oxicode::encode_to_vec(&creature).unwrap())
                .unwrap()
                .0;
        assert_eq!(decoded.palette_number, creature.palette_number);
        assert_eq!(decoded.animations.len(), 1);

        let item = ItemSheet {
            width: 40,
            height: 40,
            chunks: vec![chunk],
            frames: vec![Some(frame), None],
        };
        let decoded: ItemSheet =
            oxicode::decode_from_slice(&oxicode::encode_to_vec(&item).unwrap())
                .unwrap()
                .0;
        assert_eq!(decoded, item);

        let effect = EffectSheet {
            sheet_width: 64,
            sheet_height: 64,
            indexed: true,
            frame_interval_ms: 100,
            chunks: vec![chunk],
            frames: vec![Some(EffectSheetFrame {
                chunk: 0,
                x: 1,
                y: 2,
                width: 3,
                height: 4,
                left: -2,
                top: -1,
                center_x: 0,
                center_y: 0,
            })],
        };
        let decoded: EffectSheet =
            oxicode::decode_from_slice(&oxicode::encode_to_vec(&effect).unwrap())
                .unwrap()
                .0;
        assert_eq!(decoded, effect);
    }
}
