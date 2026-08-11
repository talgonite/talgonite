//! Light map (`.hea`) files: horizontal RLE-encoded scanline strips
//! (`(value, count)` pairs, value 0..=32) that layer together into the map's
//! per-pixel static light.

use anyhow::anyhow;
use byteorder::{LittleEndian, ReadBytesExt};
use std::io::{Cursor, Read};

#[derive(Debug, Clone, Default)]
pub struct HeaFile {
    pub screen_width: i32,
    pub screen_height: i32,
    pub tile_width: i32,
    pub tile_height: i32,
    pub scanline_width: i32,
    pub scanline_count: i32,
    pub layer_count: i32,
    /// Horizontal pixel offset of each layer strip. Length is `layer_count`.
    pub thresholds: Vec<i32>,
    /// Word offsets into `rle_data` (`layers * scanlines` entries, layer-major).
    pub scanline_offsets: Vec<i32>,
    pub rle_data: Vec<u8>,
}

impl HeaFile {
    /// The maximum light intensity value used in the RLE data.
    pub const MAX_LIGHT_VALUE: u8 = 0x20;
    /// The standard horizontal strip width for each layer (except possibly the last).
    pub const LAYER_STRIP_WIDTH: i32 = 1000;

    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let mut cursor = Cursor::new(bytes);

        let _padding = cursor.read_i32::<LittleEndian>()?;
        let screen_width = cursor.read_i32::<LittleEndian>()?;
        let screen_height = cursor.read_i32::<LittleEndian>()?;
        let _screen_width_repeat = cursor.read_i32::<LittleEndian>()?;
        let _screen_height_repeat = cursor.read_i32::<LittleEndian>()?;
        let tile_width = cursor.read_i32::<LittleEndian>()?;
        let tile_height = cursor.read_i32::<LittleEndian>()?;
        let scanline_width = cursor.read_i32::<LittleEndian>()?;
        let scanline_count = cursor.read_i32::<LittleEndian>()?;
        let layer_count = cursor.read_i32::<LittleEndian>()?;

        if layer_count <= 0 || layer_count > 1024 {
            return Err(anyhow!("invalid HEA layer count: {layer_count}"));
        }
        if scanline_count <= 0 || scanline_count > 65536 {
            return Err(anyhow!("invalid HEA scanline count: {scanline_count}"));
        }
        if scanline_width <= 0 || scanline_width > 65536 {
            return Err(anyhow!("invalid HEA scanline width: {scanline_width}"));
        }

        let mut thresholds = Vec::with_capacity(layer_count as usize);
        for _ in 0..layer_count {
            thresholds.push(cursor.read_i32::<LittleEndian>()?);
        }

        let offset_count = layer_count as usize * scanline_count as usize;
        let mut scanline_offsets = Vec::with_capacity(offset_count);
        for _ in 0..offset_count {
            scanline_offsets.push(cursor.read_i32::<LittleEndian>()?);
        }

        let mut rle_data = Vec::new();
        cursor.read_to_end(&mut rle_data)?;

        Ok(Self {
            screen_width,
            screen_height,
            tile_width,
            tile_height,
            scanline_width,
            scanline_count,
            layer_count,
            thresholds,
            scanline_offsets,
            rle_data,
        })
    }

    /// The pixel width of a layer's horizontal strip.
    pub fn layer_width(&self, layer_index: i32) -> i32 {
        let start = self.thresholds[layer_index as usize];
        let end = if layer_index + 1 < self.layer_count {
            self.thresholds[(layer_index + 1) as usize]
        } else {
            self.scanline_width
        };
        end - start
    }

    /// Decodes one layer scanline into `buffer`; bad ranges leave it
    /// zero-filled.
    pub fn decode_scanline(&self, layer_index: i32, scanline: i32, buffer: &mut [u8]) {
        let layer_width = self.layer_width(layer_index);
        let fill = layer_width.clamp(0, buffer.len() as i32) as usize;
        buffer[..fill].fill(0);

        if layer_index < 0
            || layer_index >= self.layer_count
            || scanline < 0
            || scanline >= self.scanline_count
        {
            return;
        }

        let table_index = layer_index as usize * self.scanline_count as usize + scanline as usize;
        let Some(&word_offset) = self.scanline_offsets.get(table_index) else {
            return;
        };
        if word_offset < 0 {
            return;
        }
        let mut byte_offset = word_offset as usize * 2;

        let mut pixel_index = 0usize;
        while pixel_index < fill && byte_offset + 1 < self.rle_data.len() {
            // Top two bits are flags; only the low 6 bits are intensity.
            let value = self.rle_data[byte_offset] & 0x3F;
            let count = self.rle_data[byte_offset + 1] as usize;
            byte_offset += 2;

            if count == 0 {
                continue;
            }

            let actual = count.min(fill - pixel_index);
            buffer[pixel_index..pixel_index + actual].fill(value);
            pixel_index += actual;
        }
    }

    /// Convenience wrapper returning an owned decoded scanline.
    pub fn decode_scanline_owned(&self, layer_index: i32, scanline: i32) -> Vec<u8> {
        let mut buffer = vec![0u8; self.layer_width(layer_index).max(0) as usize];
        self.decode_scanline(layer_index, scanline, &mut buffer);
        buffer
    }

    /// Decodes the full map into a row-major R8 raster.
    pub fn rasterize(&self) -> Vec<u8> {
        let width = self.scanline_width.max(0) as usize;
        let height = self.scanline_count.max(0) as usize;
        let mut out = vec![0u8; width * height];
        let mut row = vec![0u8; width];

        for layer in 0..self.layer_count {
            let layer_width = self.layer_width(layer) as usize;
            if layer_width == 0 {
                continue;
            }
            let start = self.thresholds[layer as usize].max(0) as usize;
            for y in 0..height {
                row[..layer_width].fill(0);
                self.decode_scanline(layer, y as i32, &mut row[..layer_width]);
                let dst = y * width + start;
                out[dst..dst + layer_width].copy_from_slice(&row[..layer_width]);
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_hea(scanline_count: i32, thresholds: &[i32], offsets: &[i32], rle: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for v in [
            0i32,
            640,
            480,
            640,
            480,
            100,
            100,
            6880,
            scanline_count,
            thresholds.len() as i32,
        ] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for t in thresholds {
            out.extend_from_slice(&t.to_le_bytes());
        }
        for o in offsets {
            out.extend_from_slice(&o.to_le_bytes());
        }
        out.extend_from_slice(rle);
        out
    }

    #[test]
    fn parses_header() {
        let bytes = write_hea(1, &[0, 1000], &[0, 0], &[]);
        let hea = HeaFile::from_bytes(&bytes).unwrap();
        assert_eq!(hea.screen_width, 640);
        assert_eq!(hea.screen_height, 480);
        assert_eq!(hea.scanline_width, 6880);
        assert_eq!(hea.scanline_count, 1);
        assert_eq!(hea.layer_count, 2);
        assert_eq!(hea.layer_width(0), 1000);
        assert_eq!(hea.layer_width(1), 5880);
    }

    #[test]
    fn decodes_rle_runs() {
        // Each layer's scanline fills its strip exactly: layer 0 = 5 bright + 3
        // dim + zeros; layer 1 = 4 bright + zeros.
        let rle = [
            0x20, 5, 0x01, 3, 0x00, 250, 0x00, 250, 0x00, 250, 0x00, 242, // layer 0
            0x20, 4, 0x00, 250, 0x00, 250, 0x00, 250, 0x00, 246, // layer 1
        ];
        // word offsets: layer0 scanline0 = 0, layer1 scanline0 = 6
        let bytes = write_hea(1, &[0, 1000], &[0, 6], &rle);
        let hea = HeaFile::from_bytes(&bytes).unwrap();

        let mut buf = vec![0u8; 1000];
        hea.decode_scanline(0, 0, &mut buf);
        assert_eq!(&buf[..5], &[0x20; 5]);
        assert_eq!(&buf[5..8], &[0x01; 3]);
        assert!(buf[8..].iter().all(|&v| v == 0));

        buf.fill(0);
        hea.decode_scanline(1, 0, &mut buf);
        assert_eq!(&buf[..4], &[0x20; 4]);
        assert!(buf[4..].iter().all(|&v| v == 0));
    }

    #[test]
    fn masks_flag_bits_and_handles_bad_ranges() {
        let rle = [
            0x40 | 0x20,
            2,
            0x00,
            250,
            0x00,
            250,
            0x00,
            250,
            0x00,
            246, // high bits set
            0x00,
            250,
            0x00,
            250,
            0x00,
            250,
            0x00,
            250, // layer 1
        ];
        let bytes = write_hea(1, &[0, 1000], &[0, 5], &rle);
        let hea = HeaFile::from_bytes(&bytes).unwrap();

        let mut buf = vec![0u8; 1000];
        hea.decode_scanline(0, 0, &mut buf);
        assert_eq!(&buf[..2], &[0x20; 2]);

        buf.fill(0xFF);
        hea.decode_scanline(0, 99_999, &mut buf);
        assert!(buf.iter().all(|&v| v == 0));
    }

    #[test]
    fn rejects_invalid_layer_counts() {
        let mut bytes = write_hea(1, &[0, 1000], &[0], &[]);
        bytes[36..40].copy_from_slice(&(-1i32).to_le_bytes());
        assert!(HeaFile::from_bytes(&bytes).is_err());
    }

    #[test]
    fn rasterizes_layers_into_one_image() {
        // Two layers, two scanlines: layer 0 = [32, 1, 0..], layer 1 = [0.., 16].
        let rle = [
            0x20, 1, 0x01, 1, 0x00, 248, // layer 0, scanline 0
            0x00, 250, // layer 0, scanline 1
            0x00, 249, 0x10, 1, // layer 1, scanline 0
            0x00, 250, // layer 1, scanline 1
        ];
        // 2 layers * 2 scanlines; word offsets per (layer, scanline).
        let offsets = [0i32, 3, 4, 6];
        let mut bytes = Vec::new();
        for v in [0i32, 640, 480, 640, 480, 100, 100, 1000, 2, 2] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        for t in [0i32, 500] {
            bytes.extend_from_slice(&t.to_le_bytes());
        }
        for o in offsets {
            bytes.extend_from_slice(&o.to_le_bytes());
        }
        bytes.extend_from_slice(&rle);

        let hea = HeaFile::from_bytes(&bytes).unwrap();
        let raster = hea.rasterize();
        assert_eq!(raster.len(), 1000 * 2);
        assert_eq!(raster[0], 0x20); // layer 0, scanline 0, x=0
        assert_eq!(raster[1], 0x01); // layer 0, scanline 0, x=1
        assert_eq!(raster[1000], 0); // layer 0, scanline 1
        assert_eq!(raster[749], 0x10); // layer 1, scanline 0, x=249 (threshold 500)
        assert_eq!(raster[748], 0); // layer 1, scanline 0, x=248
        assert_eq!(raster[500], 0); // layer 1, scanline 0, x=0
        assert_eq!(raster[1500], 0); // layer 1, scanline 1
    }
}
