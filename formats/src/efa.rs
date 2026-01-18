use bincode::{Decode, Encode};
use byteorder::{LE, ReadBytesExt};
use flate2::read::ZlibDecoder;
use num_enum::TryFromPrimitive;
use std::io::{Read, Seek};

#[derive(Encode, Decode, Debug, Clone)]
pub struct EfaFile {
    pub frame_interval_ms: usize,
    pub frames: Vec<EfaFrame>,
}

#[derive(TryFromPrimitive)]
#[repr(u8)]
pub enum BlendMode {
    Luminance = 1,
    LessLuminance = 2,
    Unknown = 3,
}

impl BlendMode {
    pub fn get_alpha(&self, color: (u8, u8, u8)) -> u8 {
        let coefficient = match self {
            BlendMode::Luminance => 1.,
            BlendMode::LessLuminance => 1.25,
            BlendMode::Unknown => return 255,
        };

        get_bt601_luminance(color, coefficient)
            .round()
            .clamp(0.0, 255.0) as u8
    }
}

fn get_bt601_luminance(color: (u8, u8, u8), coefficient: f32) -> f32 {
    let r_lin = (color.0 as f32 / 255.0).powi(2);
    let g_lin = (color.1 as f32 / 255.0).powi(2);
    let b_lin = (color.2 as f32 / 255.0).powi(2);

    let lum_linear = 0.299 * r_lin + 0.587 * g_lin + 0.114 * b_lin;

    lum_linear.sqrt() * 255.0 * coefficient
}

#[derive(Encode, Decode, Debug, Clone)]
pub struct EfaFrame {
    pub width: u16,
    pub height: u16,
    pub left: i16,
    pub top: i16,
    pub data: Vec<u8>,
}

impl EfaFile {
    pub fn read_from_da<R: Read + Seek>(reader: &mut R) -> anyhow::Result<Self> {
        reader.read_i32::<LE>()?; //unknown
        let frame_count = reader.read_i32::<LE>()? as usize;
        let frame_interval_ms = reader.read_i32::<LE>()? as usize;
        let blend_mode = BlendMode::try_from(reader.read_u8()?)?;

        reader.read_exact(&mut [0u8; 51])?;

        let mut frames = Vec::with_capacity(frame_count);

        for _ in 0..frame_count {
            reader.read_i32::<LE>()?; //unknown1
            let offset = reader.read_i32::<LE>()?;
            let compressed_size = reader.read_i32::<LE>()?;
            reader.read_i32::<LE>()?; //decompressedSize
            reader.read_i32::<LE>()?; //unknown2
            reader.read_i32::<LE>()?; //unknown3
            let byte_width = reader.read_i32::<LE>()?;
            reader.read_i32::<LE>()?; //unknown4
            let byte_count = reader.read_i32::<LE>()?;
            reader.read_i32::<LE>()?; //unknown5
            reader.read_i16::<LE>()?; //centerX
            reader.read_i16::<LE>()?; //centerY
            reader.read_i32::<LE>()?; //unknown6
            let width = reader.read_i16::<LE>()?;
            let height = reader.read_i16::<LE>()?;
            let left = reader.read_i16::<LE>()?;
            let top = reader.read_i16::<LE>()?;
            let _frame_width = reader.read_i16::<LE>()?;
            let _frame_height = reader.read_i16::<LE>()?;
            reader.read_i32::<LE>()?; //unknown7

            let w = width - left;
            let h = height - top;

            frames.push((
                offset,
                compressed_size,
                byte_width,
                byte_count,
                EfaFrame {
                    width: w as u16,
                    height: h as u16,
                    left,
                    top,
                    data: if byte_count > 0 {
                        vec![]
                    } else {
                        vec![0u8; w as usize * h as usize * 4]
                    },
                },
            ));
        }

        let data_start = reader.stream_position()?;

        for (offset, compressed_size, byte_width, byte_count, frame) in &mut frames {
            if *byte_count == 0 {
                continue;
            }

            reader.seek(std::io::SeekFrom::Start(data_start + *offset as u64))?;

            let mut decompressed_data = ZlibDecoder::new(reader.take(*compressed_size as u64));
            let mut buf = vec![0u8; *byte_count as usize];
            decompressed_data.read_exact(&mut buf)?;

            let rows = buf.chunks_exact(*byte_width as usize);

            frame.data = rows
                .flat_map(|row| {
                    row.chunks_exact(2)
                        .take(frame.width as usize)
                        .flat_map(|b| {
                            let rgb565 = u16::from_le_bytes([b[0], b[1]]);

                            let r = ((rgb565 >> 11) & 0x1F) as u8;
                            let g = ((rgb565 >> 5) & 0x3F) as u8;
                            let b = (rgb565 & 0x1F) as u8;

                            let r8 = (r << 3) | (r >> 2);
                            let g8 = (g << 2) | (g >> 4);
                            let b8 = (b << 3) | (b >> 2);

                            let color8 = (r8, g8, b8);
                            let a = blend_mode.get_alpha(color8);
                            [r8, g8, b8, a]
                        })
                })
                .take(frame.width as usize * frame.height as usize * 4)
                .collect();
        }

        Ok(EfaFile {
            frame_interval_ms,
            frames: frames.into_iter().map(|(_, _, _, _, f)| f).collect(),
        })
    }
}
