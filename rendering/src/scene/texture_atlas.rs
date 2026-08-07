use etagere::{AllocId, Allocation};

pub struct TextureAtlas {
    pub atlas: etagere::AtlasAllocator,
    texture: wgpu::Texture,
    bytes_per_pixel: u32,
    device: wgpu::Device,
    belt: wgpu::util::StagingBelt,
}

pub struct FrameUpload {
    /// Destination region in the atlas (may span several adjacent part slots
    /// after merging).
    pub rect: etagere::Rectangle,
    pub width: usize,
    pub height: usize,
    /// Each frame's pixels as tight rows (`width` bytes each), positioned at
    /// `y` within the part's slot.
    pub frames: Vec<FrameRow>,
}

pub struct FrameRow {
    pub x: u32,
    pub y: u32,
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

/// Counts pairs of uploads whose atlas regions could share a single
/// `copy_buffer_to_texture` today: same row pitch (aligned slot width) and
/// exactly adjacent - either side by side (same top/bottom, touching x edges)
/// or stacked (same left/right, touching y edges). Exact adjacency means the
/// merged rectangle is fully tiled by the two parts, so no third part can sit
/// between them.
fn mergeable_pair_count(uploads: &[FrameUpload]) -> (usize, usize, usize) {
    let pitch = |width: usize| (width + 255) & !255;
    let mut side_by_side = 0usize;
    let mut stacked = 0usize;
    let mut pitches = std::collections::HashSet::new();

    for upload in uploads {
        pitches.insert(pitch(upload.width));
    }

    for i in 0..uploads.len() {
        for j in (i + 1)..uploads.len() {
            let a = &uploads[i];
            let b = &uploads[j];
            if pitch(a.width) != pitch(b.width) {
                continue;
            }
            let ra = a.rect;
            let rb = b.rect;
            let same_row = ra.min.y == rb.min.y
                && ra.max.y == rb.max.y
                && (ra.max.x == rb.min.x || rb.max.x == ra.min.x);
            let same_col = ra.min.x == rb.min.x
                && ra.max.x == rb.max.x
                && (ra.max.y == rb.min.y || rb.max.y == ra.min.y);
            if same_row {
                side_by_side += 1;
            }
            if same_col {
                stacked += 1;
            }
        }
    }

    (side_by_side, stacked, pitches.len())
}

/// Collapses uploads whose atlas regions exactly tile a rectangle into a
/// single upload: same row pitch and same vertical span with touching x edges,
/// or same horizontal span with touching y edges. Frames are repositioned
/// relative to the merged region so one `copy_buffer_to_texture` covers them.
pub fn merge_uploads(mut uploads: Vec<FrameUpload>) -> Vec<FrameUpload> {
    if uploads.len() <= 1 {
        return uploads;
    }

    let (side_by_side, stacked, distinct_pitches) = mergeable_pair_count(&uploads);
    tracing::info!(
        uploads = uploads.len(),
        side_by_side_pairs = side_by_side,
        stacked_pairs = stacked,
        distinct_pitches = distinct_pitches,
        "atlas upload merge analysis"
    );

    let pitch = |width: usize| (width + 255) & !255;
    let mut merged: Vec<FrameUpload> = Vec::new();
    let mut used = vec![false; uploads.len()];

    for start in 0..uploads.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let mut group = vec![start];

        // Greedily attach any upload that shares the group's pitch and exactly
        // tiles against its current bounding box.
        loop {
            let mut min_x = i32::MAX;
            let mut max_x = i32::MIN;
            let mut min_y = i32::MAX;
            let mut max_y = i32::MIN;
            for &idx in &group {
                let r = uploads[idx].rect;
                min_x = min_x.min(r.min.x);
                max_x = max_x.max(r.max.x);
                min_y = min_y.min(r.min.y);
                max_y = max_y.max(r.max.y);
            }

            let mut grew = false;
            for j in 0..uploads.len() {
                if used[j] || pitch(uploads[j].width) != pitch(uploads[start].width) {
                    continue;
                }
                let r = uploads[j].rect;
                let same_row =
                    r.min.y == min_y && r.max.y == max_y && (r.max.x == min_x || r.min.x == max_x);
                let same_col =
                    r.min.x == min_x && r.max.x == max_x && (r.max.y == min_y || r.min.y == max_y);
                if same_row || same_col {
                    used[j] = true;
                    group.push(j);
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }

        let min_x = group.iter().map(|&i| uploads[i].rect.min.x).min().unwrap();
        let max_x = group.iter().map(|&i| uploads[i].rect.max.x).max().unwrap();
        let min_y = group.iter().map(|&i| uploads[i].rect.min.y).min().unwrap();
        let max_y = group.iter().map(|&i| uploads[i].rect.max.y).max().unwrap();

        let mut frames = Vec::new();
        for &idx in &group {
            let upload = &mut uploads[idx];
            let dx = (upload.rect.min.x - min_x) as u32;
            let dy = (upload.rect.min.y - min_y) as u32;
            for mut frame in std::mem::take(&mut upload.frames) {
                frame.x += dx;
                frame.y += dy;
                frames.push(frame);
            }
        }

        merged.push(FrameUpload {
            rect: etagere::Rectangle {
                min: etagere::Point::new(min_x, min_y),
                max: etagere::Point::new(max_x, max_y),
            },
            width: (max_x - min_x) as usize,
            height: (max_y - min_y) as usize,
            frames,
        });
    }

    merged
}

impl TextureAtlas {
    pub fn new(device: &wgpu::Device, texture: wgpu::Texture) -> Self {
        Self {
            atlas: etagere::AtlasAllocator::new(etagere::size2(
                texture.width() as i32,
                texture.height() as i32,
            )),
            bytes_per_pixel: texture.format().block_copy_size(None).unwrap_or_default(),
            texture,
            device: device.clone(),
            // Ring of reusable staging buffers sized for whole-batch map loads.
            belt: wgpu::util::StagingBelt::new(device.clone(), 16 * 1024 * 1024),
        }
    }

    /// Reserves a slot in the atlas without uploading anything.
    #[tracing::instrument(level = "info", skip(self), fields(width, height))]
    pub fn allocate_slot(&mut self, width: usize, height: usize) -> Option<Allocation> {
        self.atlas
            .allocate(etagere::size2(width as i32, height as i32))
    }

    /// Uploads many frames in one submit: all data is packed into staging belt
    /// slices (rows padded to `COPY_BUFFER_ALIGNMENT`), then one command
    /// encoder issues a `copy_buffer_to_texture` per frame and the queue gets a
    /// single submit.
    #[tracing::instrument(level = "info", skip_all, fields(
        upload_count = uploads.len(),
        actual_bytes = uploads.iter().flat_map(|u| u.frames.iter()).map(|f| f.data.len()).sum::<usize>(),
        padded_bytes = uploads.iter().map(|u| u.width * u.height).sum::<usize>(),
    ))]
    pub fn upload_batch(&mut self, queue: &wgpu::Queue, uploads: &[FrameUpload]) {
        if uploads.is_empty() {
            return;
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let bpp = self.bytes_per_pixel as usize;

        for upload in uploads {
            let row_bytes = bpp * upload.width;
            let aligned_row = (row_bytes + alignment - 1) & !(alignment - 1);
            let size = wgpu::BufferSize::new((aligned_row * upload.height) as u64).unwrap();
            let slice = self.belt.allocate(
                size,
                wgpu::BufferSize::new(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT.into()).unwrap(),
            );

            {
                let mut view = slice.get_mapped_range_mut();
                for frame in &upload.frames {
                    // Frames may be indexed (R8) or RGBA, so row length and x
                    // offset are in bytes, not pixels.
                    let frame_row_bytes = bpp * frame.width;
                    debug_assert_eq!(frame.data.len(), frame_row_bytes * frame.height);
                    let base = frame.y as usize * aligned_row + frame.x as usize * bpp;
                    for (row, chunk) in frame.data.chunks_exact(frame_row_bytes).enumerate() {
                        let start = base + row * aligned_row;
                        view.slice(start..start + frame_row_bytes)
                            .copy_from_slice(chunk);
                    }
                }
            }

            let origin = upload.rect.min;
            encoder.copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: slice.buffer(),
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: slice.offset(),
                        bytes_per_row: Some(aligned_row as u32),
                        rows_per_image: None,
                    },
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: origin.x as u32,
                        y: origin.y as u32,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: upload.width as u32,
                    height: upload.height as u32,
                    depth_or_array_layers: 1,
                },
            );
        }

        self.belt.finish();
        queue.submit([encoder.finish()]);
        self.belt.recall();
    }

    pub fn deallocate(&mut self, id: AllocId) {
        self.atlas.deallocate(id);
    }
}
