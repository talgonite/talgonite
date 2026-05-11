use formats::ktx2;
use formats::spf::SpfFile;
use std::io::{Cursor, Read};
use std::path::Path;

use crate::asset_record::AssetRecord;

#[derive(Default)]
pub(crate) struct TextureAssetProcessor;

impl TextureAssetProcessor {
    pub(crate) fn try_process(
        &self,
        dat_path: &Path,
        file_name: &str,
        file_size: usize,
        entry_reader: &mut dyn Read,
    ) -> anyhow::Result<Option<Vec<AssetRecord>>> {
        if file_name == "tilea.bmp" || file_name == "tileas.bmp" {
            let tilea_name = file_name.trim_end_matches(".bmp");
            return Ok(Some(self.process_tile_pages(dat_path, tilea_name, file_size, entry_reader)?));
        }

        if file_name.ends_with(".hpf") {
            return Ok(Some(vec![self.process_hpf(dat_path, file_name, file_size, entry_reader)?]));
        }

        if file_name.ends_with(".spf") {
            return Ok(Some(self.process_spf(dat_path, file_name, file_size, entry_reader)?));
        }

        Ok(None)
    }

    fn process_tile_pages(
        &self,
        dat_path: &Path,
        tilea_name: &str,
        file_size: usize,
        entry_reader: &mut dyn Read,
    ) -> anyhow::Result<Vec<AssetRecord>> {
        const TILE_WIDTH: usize = 56;
        const TILE_HEIGHT: usize = 27;
        const TILES_PER_ROW: usize = 128;
        const TILE_ROWS_PER_PAGE: usize = 5;
        const PAGE_WIDTH: usize = TILES_PER_ROW * TILE_WIDTH;
        const TILE_SIZE: usize = TILE_WIDTH * TILE_HEIGHT;

        let mut tiles_remaining = file_size / TILE_SIZE;
        let mut page_index: usize = 0;
        let mut records = Vec::new();

        while tiles_remaining > 0 {
            let mut row_tile_counts: Vec<usize> = Vec::new();
            for _ in 0..TILE_ROWS_PER_PAGE {
                if tiles_remaining == 0 {
                    break;
                }
                let row_tiles = tiles_remaining.min(TILES_PER_ROW);
                if row_tiles == 0 {
                    break;
                }
                row_tile_counts.push(row_tiles);
                tiles_remaining -= row_tiles;
            }

            let rows_this_page = row_tile_counts.len();
            let tiles_for_page: usize = row_tile_counts.iter().sum();
            let mut page_buffer = vec![0u8; PAGE_WIDTH * (rows_this_page * TILE_HEIGHT)];

            let mut tiles_read_in_page = 0usize;
            for row in 0..rows_this_page {
                let remaining_for_page = tiles_for_page - tiles_read_in_page;
                let row_tiles = row_tile_counts[row].min(remaining_for_page);

                let mut tile_data: Vec<[u8; TILE_SIZE]> = Vec::with_capacity(row_tiles);
                for _ in 0..row_tiles {
                    let mut buf = [0u8; TILE_SIZE];
                    entry_reader.read_exact(&mut buf)?;
                    tile_data.push(buf);
                }

                for y in 0..TILE_HEIGHT {
                    let dest_row_start = (row * TILE_HEIGHT + y) * PAGE_WIDTH;
                    let mut dest_offset = dest_row_start;

                    for tile_buf in &tile_data {
                        let src_start = y * TILE_WIDTH;
                        let src_end = src_start + TILE_WIDTH;
                        page_buffer[dest_offset..dest_offset + TILE_WIDTH]
                            .copy_from_slice(&tile_buf[src_start..src_end]);
                        dest_offset += TILE_WIDTH;
                    }

                    let remaining_tiles = TILES_PER_ROW - row_tiles;
                    if remaining_tiles > 0 {
                        let pad_bytes = remaining_tiles * TILE_WIDTH;
                        for byte in &mut page_buffer[dest_offset..dest_offset + pad_bytes] {
                            *byte = 0;
                        }
                    }
                }

                tiles_read_in_page += row_tiles;
            }

            let page_pixel_height = rows_this_page * TILE_HEIGHT;
            let ktx_header = ktx2::get_ktx2_header(
                PAGE_WIDTH as u32,
                page_pixel_height as u32,
                ktx2::VK_FORMAT_R8_UNORM,
                (PAGE_WIDTH * page_pixel_height) as u64,
            )?;

            let page_name = format!("{}_{:03}.ktx2", tilea_name, page_index);
            records.push(AssetRecord::chunks(
                dat_path.join(page_name),
                vec![ktx_header, page_buffer],
            ));
            page_index += 1;
        }

        Ok(records)
    }

    fn process_hpf(
        &self,
        dat_path: &Path,
        file_name: &str,
        file_size: usize,
        entry_reader: &mut dyn Read,
    ) -> anyhow::Result<AssetRecord> {
        let mut file_buffer = vec![0u8; file_size];
        entry_reader.read_exact(&mut file_buffer)?;

        let signature = u32::from_le_bytes(file_buffer[0..4].try_into().unwrap());
        let buf = if signature != 0xFF02AA55 {
            file_buffer[8..].to_vec()
        } else {
            formats::hpf::decompress(&file_buffer)[8..].to_vec()
        };
        let hpf_ktx2 = ktx2::get_ktx2_header(
            28,
            buf.len() as u32 / 28,
            ktx2::VK_FORMAT_R8_UNORM,
            buf.len() as _,
        )?;

        Ok(AssetRecord::chunks(
            dat_path.join(file_name.replace(".hpf", ".ktx2")),
            vec![hpf_ktx2, buf],
        ))
    }

    fn process_spf(
        &self,
        dat_path: &Path,
        file_name: &str,
        file_size: usize,
        entry_reader: &mut dyn Read,
    ) -> anyhow::Result<Vec<AssetRecord>> {
        let mut file_buffer = vec![0u8; file_size];
        entry_reader.read_exact(&mut file_buffer)?;

        let mut reader = Cursor::new(file_buffer);
        let mut records = Vec::new();

        match SpfFile::read_from_da(&mut reader) {
            Ok(spf) => {
                let base_name = file_name.trim_end_matches(".spf");

                for (frame_idx, frame) in spf.frames.iter().enumerate() {
                    if frame.width == 0 || frame.height == 0 {
                        continue;
                    }

                    let ktx_header = ktx2::get_ktx2_header(
                        frame.width,
                        frame.height,
                        ktx2::VK_FORMAT_R8G8B8A8_UNORM,
                        frame.data.len() as u64,
                    )?;

                    let frame_name = format!("{}.{}.ktx2", base_name, frame_idx);
                    records.push(AssetRecord::chunks(
                        dat_path.join(frame_name),
                        vec![ktx_header, frame.data.clone()],
                    ));
                }
            }
            Err(error) => {
                tracing::warn!("Failed to read SPF file {}: {:?}", file_name, error);
            }
        }

        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::TextureAssetProcessor;
    use std::io::Cursor;
    use std::path::Path;

    #[test]
    fn try_process_only_handles_texture_entries() {
        let processor = TextureAssetProcessor;
        let mut reader = Cursor::new(Vec::<u8>::new());

        let result = processor
            .try_process(Path::new("Legend"), "foo.txt", 0, &mut reader)
            .unwrap();

        assert!(result.is_none());
    }
}