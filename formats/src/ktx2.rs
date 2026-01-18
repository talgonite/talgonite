pub const VK_FORMAT_R8_UNORM: u32 = 9;
pub const VK_FORMAT_R8G8B8A8_UNORM: u32 = 37;

use std::{io::Write, mem};

use byteorder::{LE, WriteBytesExt};
use vk2dfd::vk2dfd;

const IDENTIFIER: [u8; 12] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];

pub fn get_ktx2_header(
    width: u32,
    height: u32,
    format: u32,
    data_len: u64,
) -> anyhow::Result<Vec<u8>> {
    let format_header = [
        format, // vkFormat
        1,      // typeSize
        width,  // width
        height, // height
        1,      // depth
        0,      // layerCount
        1,      // faceCount
        1,      // levelCount
        0,      // supercompressionScheme
    ];

    let dfd = vk2dfd(format)?;

    let mut index_header = [
        0,                            // dfdByteOffset
        mem::size_of_val(dfd) as u32, // dfdByteLength
        0,                            // kvdByteOffset
        0,                            // kvdByteLength
    ];

    let mut level_index = [
        0,        // sgdByteOffset
        0,        // sgdByteLength
        0,        // byteOffset
        data_len, // byteLength
        data_len, // uncompressedByteLength
    ];

    let dfd_byte_offset = mem::size_of_val(&IDENTIFIER)
        + mem::size_of_val(&format_header)
        + mem::size_of_val(&index_header)
        + mem::size_of_val(&level_index);
    index_header[0] = dfd_byte_offset as u32;

    // Align up.
    let mut level_byte_offset = dfd_byte_offset + mem::size_of_val(dfd);
    let padding_before_data = level_byte_offset % 4;
    level_byte_offset += padding_before_data;
    level_index[2] = level_byte_offset as u64;

    let mut buf: Vec<u8> = Vec::with_capacity(level_byte_offset as usize);

    buf.write_all(&IDENTIFIER)?;
    for value in format_header.into_iter() {
        buf.write_u32::<LE>(value)?;
    }
    for value in index_header.into_iter() {
        buf.write_u32::<LE>(value)?;
    }
    for value in level_index.into_iter() {
        buf.write_u64::<LE>(value)?;
    }
    for &word in dfd {
        buf.write_u32::<LE>(word)?;
    }
    for _ in 0..padding_before_data {
        buf.write_u8(0)?;
    }

    Ok(buf)
}
