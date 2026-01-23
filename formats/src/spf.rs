use byteorder::{LE, ReadBytesExt};
use std::io::{Read, Seek, SeekFrom};

/// SPF file format parser
/// SPF files contain sprite frames, either palettized or colorized (RGB565)
pub struct SpfFile {
    pub frames: Vec<SpfFrame>,
}

pub struct SpfFrame {
    pub width: u32,
    pub height: u32,
    pub left: i16,
    pub top: i16,
    /// RGBA8 pixel data
    pub data: Vec<u8>,
}

impl SpfFile {
    pub fn read_from_da<R: Read + Seek>(reader: &mut R) -> anyhow::Result<Self> {
        let _unknown1 = reader.read_u32::<LE>()?;
        let _unknown2 = reader.read_u32::<LE>()?;
        let format = reader.read_u32::<LE>()?;

        // Format determines if we have palettes or direct color
        // 0 = colorized (RGB565), non-zero = palettized
        let is_palettized = format != 0;

        let palette: Option<Vec<(u8, u8, u8)>> = if is_palettized {
            // Read primary palette (256 entries, RGB565)
            let mut palette = Vec::with_capacity(256);
            for _ in 0..256 {
                let rgb565 = reader.read_u16::<LE>()?;
                let (r8, g8, b8) = rgb565_to_rgb8(rgb565);
                palette.push((r8, g8, b8));
            }
            // Skip secondary palette (256 entries, RGB555) - not used
            reader.seek(SeekFrom::Current(256 * 2))?;
            Some(palette)
        } else {
            None
        };

        let frame_count = reader.read_u32::<LE>()? as usize;

        if frame_count == 0 {
            return Ok(SpfFile { frames: vec![] });
        }

        // Read frame headers
        let mut frame_headers = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            let left = reader.read_u16::<LE>()? as i16;
            let top = reader.read_u16::<LE>()? as i16;
            let right = reader.read_u16::<LE>()?;
            let bottom = reader.read_u16::<LE>()?;
            let _unknown = reader.read_u32::<LE>()?;
            let _reserved = reader.read_u32::<LE>()?;
            let start_address = reader.read_u32::<LE>()?;
            let _byte_width = reader.read_u32::<LE>()?;
            let byte_count = reader.read_u32::<LE>()?;
            let _image_byte_count = reader.read_u32::<LE>()?;

            let width = right.saturating_sub(left as u16) as u32;
            let height = bottom.saturating_sub(top as u16) as u32;

            frame_headers.push((left, top, width, height, start_address, byte_count));
        }

        // Read data section size
        let _data_size = reader.read_u32::<LE>()?;
        let data_start = reader.stream_position()?;

        // Read frame data
        let mut frames = Vec::with_capacity(frame_count);
        for (left, top, width, height, start_address, byte_count) in frame_headers {
            if width == 0 || height == 0 || byte_count == 0 {
                frames.push(SpfFrame {
                    width,
                    height,
                    left,
                    top,
                    data: vec![0u8; (width * height * 4) as usize],
                });
                continue;
            }

            reader.seek(SeekFrom::Start(data_start + start_address as u64))?;

            let rgba_data = if let Some(ref pal) = palette {
                // Palettized: one byte per pixel, look up in palette
                let mut indices = vec![0u8; byte_count as usize];
                reader.read_exact(&mut indices)?;

                indices
                    .iter()
                    .flat_map(|&idx| {
                        let (r, g, b) = pal[idx as usize];
                        // Index 0 is typically transparent
                        let a = if idx == 0 { 0 } else { 255 };
                        [r, g, b, a]
                    })
                    .collect()
            } else {
                // Colorized: RGB565 data (2 bytes per pixel)
                // The file stores RGB565 primary copy, then RGB555 secondary copy
                // We only need the RGB565 part
                let pixel_count = (width * height) as usize;
                let mut rgba = Vec::with_capacity(pixel_count * 4);

                for _ in 0..pixel_count {
                    let rgb565 = reader.read_u16::<LE>()?;
                    let (r8, g8, b8) = rgb565_to_rgb8(rgb565);
                    // Treat black (0,0,0) as transparent for colorized sprites
                    let a = if rgb565 == 0 { 0 } else { 255 };
                    rgba.extend_from_slice(&[r8, g8, b8, a]);
                }

                rgba
            };

            frames.push(SpfFrame {
                width,
                height,
                left,
                top,
                data: rgba_data,
            });
        }

        Ok(SpfFile { frames })
    }
}

/// Convert RGB565 to RGB888
fn rgb565_to_rgb8(rgb565: u16) -> (u8, u8, u8) {
    let r = ((rgb565 >> 11) & 0x1F) as u8;
    let g = ((rgb565 >> 5) & 0x3F) as u8;
    let b = (rgb565 & 0x1F) as u8;

    // Expand to 8-bit with proper bit replication
    let r8 = (r << 3) | (r >> 2);
    let g8 = (g << 2) | (g >> 4);
    let b8 = (b << 3) | (b >> 2);

    (r8, g8, b8)
}
