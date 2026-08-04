use crate::texture;
use formats::palette::AnimatedPaletteRange;
use std::time::{Duration, Instant};
use wgpu;

/// Tracks the rotation schedule of a single animated range. Each range in a
/// slot declares its own period, so timers are kept per range instead of
/// sharing a slot-wide period derived from the first range.
struct AnimatedPaletteRangeTimer {
    range: AnimatedPaletteRange,
    period: Duration,
    next_update: Instant,
}

impl AnimatedPaletteRangeTimer {
    fn new(range: AnimatedPaletteRange, now: Instant) -> Self {
        // A period of zero is treated as one millisecond to keep the update
        // from dividing by zero; it still animates on every frame.
        let period_ms = (u64::from(range.period) * 100).max(1);
        let period = Duration::from_millis(period_ms);

        Self {
            range,
            period,
            next_update: now + period,
        }
    }

    /// Returns how many rotation steps are due at `now`. If a frame takes
    /// longer than the period, all missed steps are returned and the timer is
    /// caught up against the original schedule, so the animation does not lag
    /// behind or drift out of phase.
    fn due_steps(&mut self, now: Instant) -> u32 {
        if now < self.next_update {
            return 0;
        }

        let period_ms = self.period.as_millis() as u64;
        let elapsed_ms = now.duration_since(self.next_update).as_millis() as u64;
        let steps = elapsed_ms / period_ms + 1;

        self.next_update += Duration::from_millis(period_ms * steps);
        steps as u32
    }
}

/// Rotates one or more color ranges of a palette row on their own intervals.
/// This is the mechanism used by the official client for water/fountain
/// animation: instead of swapping tile frames, the colors inside the palette
/// row are shifted, which animates every tile that references that palette.
struct AnimatedPaletteSlot {
    ranges: Vec<AnimatedPaletteRangeTimer>,
}

impl AnimatedPaletteSlot {
    fn new(ranges: Vec<AnimatedPaletteRange>, now: Instant) -> Self {
        Self {
            ranges: ranges
                .into_iter()
                .map(|range| AnimatedPaletteRangeTimer::new(range, now))
                .collect(),
        }
    }

    /// Advances every range that is due and returns the number of steps each
    /// range should rotate, aligned with `ranges`.
    fn due_steps(&mut self, now: Instant) -> Vec<u32> {
        self.ranges
            .iter_mut()
            .map(|timer| timer.due_steps(now))
            .collect()
    }
}

/// Rotates a single range `shift` steps to the right. The row is mutated in
/// place, so each range returns to its original layout after
/// `end - start + 1` advances.
fn rotate_range(range: &AnimatedPaletteRange, row: &mut [u8], shift: u32) {
    let start = usize::from(range.start_index);
    let end = usize::from(range.end_index);
    let len = end - start + 1;

    if shift == 0 || len <= 1 {
        return;
    }

    let shift = (shift as usize) % len;
    let colors = row[start * 4..end * 4 + 4].to_vec();

    for i in 0..len {
        let src = (i + len - shift) % len;
        row[(start + i) * 4..(start + i) * 4 + 4].copy_from_slice(&colors[src * 4..src * 4 + 4]);
    }
}

/// A palette texture whose referenced rows are animated over time. Rows that
/// are not used by the current map are left untouched.
pub struct AnimatedPaletteTexture {
    pub texture: texture::Texture,
    palette_width: u32,
    pixels: Vec<u8>,
    slots: Vec<(u16, AnimatedPaletteSlot)>,
}

impl AnimatedPaletteTexture {
    pub fn new(
        texture: texture::Texture,
        pixels: Vec<u8>,
        palette_width: u32,
        animated_palettes: Vec<(u16, Vec<AnimatedPaletteRange>)>,
        now: Instant,
    ) -> Self {
        let slots = animated_palettes
            .into_iter()
            .map(|(row, ranges)| (row, AnimatedPaletteSlot::new(ranges, now)))
            .collect();

        Self {
            texture,
            palette_width,
            pixels,
            slots,
        }
    }

    pub fn update(&mut self, queue: &wgpu::Queue, now: Instant) {
        let mut rows_to_update = Vec::new();

        for (row, slot) in &mut self.slots {
            let steps = slot.due_steps(now);
            if steps.iter().any(|&step| step > 0) {
                rows_to_update.push((*row, steps));
            }
        }

        if rows_to_update.is_empty() {
            return;
        }

        let row_bytes = self.palette_width as usize * 4;

        for (row, steps) in rows_to_update {
            let start = usize::from(row) * row_bytes;
            let end = start + row_bytes;

            if end > self.pixels.len() {
                continue;
            }

            if let Some((_, slot)) = self.slots.iter().find(|(slot_row, _)| *slot_row == row) {
                for (timer, &step) in slot.ranges.iter().zip(&steps) {
                    rotate_range(&timer.range, &mut self.pixels[start..end], step);
                }
            }

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    aspect: wgpu::TextureAspect::All,
                    texture: &self.texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: u32::from(row),
                        z: 0,
                    },
                },
                &self.pixels[start..end],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_bytes as u32),
                    rows_per_image: Some(1),
                },
                wgpu::Extent3d {
                    width: self.palette_width,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
}
