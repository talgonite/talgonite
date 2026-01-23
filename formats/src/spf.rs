use byteorder::{LE, ReadBytesExt};
use std::io::{Read, Seek, SeekFrom};

pub struct SpfFile {
    pub frames: Vec<SpfFrame>,
}

pub struct SpfFrame {
    pub width: u32,
    pub height: u32,
    pub left: i16,
    pub top: i16,
    pub data: Vec<u8>,
}

impl SpfFile {
    pub fn read_from_da<R: Read + Seek>(reader: &mut R) -> anyhow::Result<Self> {
        let _unknown1 = reader.read_u32::<LE>()?;
        let _unknown2 = reader.read_u32::<LE>()?;
        let format = reader.read_u32::<LE>()?;

        // Format: 0 = Palettized, 2 = Colorized
        let palette = if format == 0 {
            let mut pal = Vec::with_capacity(256);
            for _ in 0..256 {
                let rgb565 = reader.read_u16::<LE>()?;
                pal.push(rgb565_to_rgb8(rgb565));
            }
            reader.seek(SeekFrom::Current(512))?;
            Some(pal)
        } else {
            None
        };

        let frame_count = reader.read_u32::<LE>()? as usize;

        if frame_count > 10000 {
            anyhow::bail!("frame_count {} is too large", frame_count);
        }

        let mut frame_headers = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            let left = reader.read_u16::<LE>()?;
            let top = reader.read_u16::<LE>()?;
            let right = reader.read_u16::<LE>()?;
            let bottom = reader.read_u16::<LE>()?;
            let _reserved = reader.read_u32::<LE>()?;
            let _reserved2 = reader.read_u32::<LE>()?;
            let start_address = reader.read_u32::<LE>()? as usize;
            let byte_width = reader.read_u32::<LE>()? as usize;
            let byte_count = reader.read_u32::<LE>()? as usize;
            let _image_byte_count = reader.read_u32::<LE>()?;

            let width = right.saturating_sub(left) as u32;
            let height = bottom.saturating_sub(top) as u32;

            frame_headers.push((left as i16, top as i16, width, height, start_address, byte_width, byte_count));
        }

        let _total_byte_count = reader.read_u32::<LE>()?;
        let data_start = reader.stream_position()? as usize;

        let mut frames = Vec::with_capacity(frame_count);
        for (left, top, width, height, start_address, byte_width, byte_count) in frame_headers {
            if width == 0 || height == 0 {
                frames.push(SpfFrame { width, height, left, top, data: vec![] });
                continue;
            }

            reader.seek(SeekFrom::Start((data_start + start_address) as u64))?;

            let rgba = if let Some(ref pal) = palette {
                let mut indices = vec![0u8; byte_count];
                reader.read_exact(&mut indices)?;
                let mut rgba = Vec::with_capacity((width * height * 4) as usize);
                for row in indices.chunks(byte_width.max(1)) {
                    for &idx in row.iter().take(width as usize) {
                        let (r, g, b) = pal[idx as usize];
                        let a = if idx == 0 { 0 } else { 255 };
                        rgba.extend_from_slice(&[r, g, b, a]);
                    }
                }
                rgba
            } else {
                let bytes_needed = (width * height * 2) as usize;
                let mut pixel_data = vec![0u8; bytes_needed];
                reader.read_exact(&mut pixel_data)?;
                let mut rgba = Vec::with_capacity((width * height * 4) as usize);
                for chunk in pixel_data.chunks_exact(2) {
                    let rgb565 = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let (r, g, b) = rgb565_to_rgb8(rgb565);
                    let a = if rgb565 == 0 { 0 } else { 255 };
                    rgba.extend_from_slice(&[r, g, b, a]);
                }
                rgba
            };

            frames.push(SpfFrame { width, height, left, top, data: rgba });
        }

        Ok(SpfFile { frames })
    }
}

fn rgb565_to_rgb8(rgb565: u16) -> (u8, u8, u8) {
    let r = ((rgb565 >> 11) & 0x1F) as u8;
    let g = ((rgb565 >> 5) & 0x3F) as u8;
    let b = (rgb565 & 0x1F) as u8;
    ((r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2))
}
