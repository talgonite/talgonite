const INSTALLER_URL: &str = "https://s3.amazonaws.com/kru-downloads/da/DarkAges741single.exe";

use byteorder::{LE, ReadBytesExt};
use circbuf::CircBuf;
use crc32fast::Hasher;
use flate2::bufread::DeflateDecoder;
use formats::efa::EfaFile;
use formats::ktx2;
use formats::spf::SpfFile;
use formats::{
    epf::{EpfFrame, EpfImage},
    mpf::MpfFile,
};
use jubako::{self as jbk, creator::ContentAdder};
use libarx::{self as arx, CreatorError, FullBuilder};
use rangemap::RangeMap;
use std::{
    io::{self, BufRead, BufReader, Cursor, Read, Write},
    path::Path,
    rc::Rc,
    sync::Arc,
};
use tracing::{debug, info};

const HEADER_SIZE_TO_SKIP: u64 = 1024 * 50;
const VERSION_BUF: &[u8] = b"741_2";

pub trait InstallProgress: Send + Sync {
    fn report(&self, percent: f32, message: String);
}

pub fn install(output: &Path, progress: Option<Arc<dyn InstallProgress>>) -> anyhow::Result<()> {
    if let Some(p) = &progress {
        p.report(0.0, "Checking archive...".to_string());
    }
    if output.exists() {
        let existing_archive = libarx::Arx::new(output).unwrap();

        let version_file =
            match existing_archive.get_entry::<FullBuilder>(arx::Path::new("VERSION")) {
                Ok(arx::Entry::File(content_address)) => {
                    match existing_archive.get_bytes(content_address.content()) {
                        Ok(Some(jbk::reader::MayMissPack::FOUND(Some(bytes)))) => {
                            let mut buf = vec![];
                            bytes.stream().read_to_end(&mut buf).unwrap();
                            Some(buf)
                        }
                        _ => None,
                    }
                }
                _ => None,
            };

        if let Some(version_file) = version_file {
            if version_file == VERSION_BUF {
                info!("Archive is up to date");
                return Ok(());
            }
        }

        info!("Archive is not up to date, updating");
    } else {
        info!("Archive does not exist, creating");
    }

    #[cfg(feature = "exe")]
    let executable_offset = {
        let pe = exe::VecPE::from_disk_file(INSTALLER_PATH).unwrap();

        let resource_section = pe.get_section_by_name(".rsrc").unwrap();
        let executable_offset =
            resource_section.pointer_to_raw_data.0 + resource_section.size_of_raw_data;
        executable_offset as u64
    };
    #[cfg(not(feature = "exe"))]
    let executable_offset = 0x3A00;

    let output_dir = output.parent().unwrap();
    let exe_file = if output_dir.join("DarkAges741single.exe").exists() {
        debug!("Using local DarkAges741single.exe");
        let file = std::fs::File::open(output_dir.join("DarkAges741single.exe"))?;
        ExeReader::File(file)
    } else {
        debug!("Streaming DarkAges741single.exe from {}", INSTALLER_URL);
        let response = reqwest::blocking::get(INSTALLER_URL)?;
        ExeReader::Http(response)
    };

    let mut exe_reader = BufReader::new(exe_file);
    let mut header_reader = (&mut exe_reader).take(HEADER_SIZE_TO_SKIP);

    header_reader.seek_relative(executable_offset)?;
    let wise_header = read_wise_overlay_header(&mut header_reader);

    header_reader.seek_relative(wise_header.dib_compressed_size as i64)?;

    let mut script: Vec<u8> =
        Vec::with_capacity(wise_header.wise_script_uncompressed_size as usize);
    DeflateDecoder::new(&mut header_reader).read_to_end(&mut script)?;

    let _crc = header_reader.read_u32::<LE>().unwrap();

    // Consume the rest of the header
    io::copy(&mut header_reader, &mut io::sink())?;
    let mut exe_reader = header_reader.into_inner();
    let mut exe_reader_position = HEADER_SIZE_TO_SKIP;

    assert!(script.len() == wise_header.wise_script_uncompressed_size as usize);

    let mut crc32_buffer = vec![0u8; 4];

    let mut reader = Cursor::new(script);
    read_header(&mut reader);
    read_languages(&mut reader)?;
    let mut operations = vec![];

    while let Ok(operation) = read_operation(&mut reader) {
        operations.push(operation);
    }

    let last_deflate_end = operations
        .iter()
        .map(|op| match op {
            Operation::CreateFile(file_header) => file_header.deflate_end,
            Operation::UnknownFile(deflate_end) => *deflate_end,
            _ => 0,
        })
        .max()
        .unwrap();
    let file_data_start = (wise_header.eof - last_deflate_end) as u64;

    let mut dat_buffer = CircBuf::with_capacity(8192)?;
    let mut buffer = vec![0u8; 4096];

    let mut arx_creator = libarx::create::SimpleCreator::new(
        jbk::Utf8Path::new(output.to_str().unwrap()),
        jbk::creator::ConcatMode::OneFile,
        Arc::new(()),
        Rc::new(()),
        jbk::creator::Compression::zstd(),
    )?;

    let total_compressed_size: u64 = operations
        .iter()
        .filter_map(|op| match op {
            Operation::CreateFile(file_header) => {
                let is_dat = file_header.file_path.ends_with(".dat");
                let is_music = file_header.file_path.ends_with(".mus");
                if is_dat || is_music {
                    Some((file_header.deflate_end - file_header.deflate_start - 4) as u64)
                } else {
                    None
                }
            }
            _ => None,
        })
        .sum();

    let mut processed_compressed_size: u64 = 0;
    for op in operations {
        if let Operation::CreateFile(file_header) = op {
            let is_dat = file_header.file_path.ends_with(".dat");
            let is_music = file_header.file_path.ends_with(".mus");

            if !is_dat && !is_music {
                continue;
            }

            let file_size = (file_header.deflate_end - file_header.deflate_start - 4) as u64;

            if let Some(p) = &progress {
                let extract_p = if total_compressed_size > 0 {
                    (processed_compressed_size as f32) / (total_compressed_size as f32)
                } else {
                    (processed_compressed_size as f32) / 200_000_000.0
                };
                p.report(
                    extract_p,
                    format!(
                        "Extracting {} ({:.1}%)",
                        file_header.file_path,
                        extract_p * 100.0
                    ),
                );
            }

            let dat_path = file_header.file_path.replace(".dat", "");
            let dat_path = Path::new(&dat_path);

            let data_start = file_data_start + (file_header.deflate_start as u64);

            assert!(exe_reader_position <= data_start);
            exe_reader.seek_relative((data_start - exe_reader_position) as i64)?;
            exe_reader_position = data_start;

            {
                let mut hasher = Hasher::new();

                {
                    let mut file_reader = exe_reader.take(file_size);
                    let mut decoder = DeflateDecoder::new(&mut file_reader);

                    if is_dat {
                        dat_buffer.clear();
                        let mut files_to_process: Vec<(String, Vec<u8>)> = Vec::new();
                        let mut epfs_to_concat: Vec<(String, EpfImage)> = Vec::new();

                        let mut files_in_dat: Vec<DatFileEntry> = Vec::new();
                        let mut file_count: Option<u32> = None;

                        let dat_name = dat_path.file_name().unwrap().to_string_lossy();
                        debug!("Extracting dat: {}", dat_name);

                        while let Ok(bytes_read) = decoder.read(&mut buffer) {
                            if bytes_read > 0 {
                                let buf = &buffer[..bytes_read];
                                hasher.update(&buf);

                                if bytes_read > dat_buffer.avail() {
                                    dat_buffer.grow()?;
                                }
                                dat_buffer.write(&buf)?;
                            }
                            // ... (rest of dat processing)

                            if files_in_dat.is_empty() {
                                let file_count = match file_count {
                                    Some(c) => c,
                                    None => {
                                        let c = dat_buffer.read_u32::<LE>()?;
                                        file_count = Some(c);
                                        c
                                    }
                                };

                                let bytes_required = file_count * 17;

                                if dat_buffer.len() < bytes_required as usize {
                                    continue;
                                }

                                for i in 0..file_count {
                                    let offset = dat_buffer.read_u32::<LE>()?;

                                    let mut name_buf = [0u8; 13];
                                    dat_buffer.read_exact(&mut name_buf).unwrap();
                                    let null_index = memchr::memchr(b'\0', &name_buf).unwrap_or(13);

                                    if null_index == 0 {
                                        continue;
                                    }

                                    let name =
                                        String::from_utf8_lossy(&name_buf[..(null_index as usize)])
                                            .trim_end()
                                            .to_lowercase();

                                    if name.len() == 0 {
                                        continue;
                                    }

                                    let is_last_file = i == (file_count - 1);
                                    let size = if is_last_file {
                                        0
                                    } else {
                                        let next_offset =
                                            dat_buffer.reader_peek().read_u32::<LE>()?;
                                        (next_offset - offset) as usize
                                    };

                                    files_in_dat.push(DatFileEntry { name, size });
                                }

                                files_in_dat.reverse();
                            } else {
                                while let Some(file) = files_in_dat.pop() {
                                    if dat_buffer.len() < file.size {
                                        files_in_dat.push(file);
                                        break;
                                    }

                                    if file.name == "tilea.bmp" || file.name == "tileas.bmp" {
                                        let tilea_name = file.name.trim_end_matches(".bmp");
                                        const TILE_WIDTH: usize = 56;
                                        const TILE_HEIGHT: usize = 27;

                                        const TILES_PER_ROW: usize = 128; // 128 tiles wide
                                        const TILE_ROWS_PER_PAGE: usize = 5; // 5 tiles high per page

                                        const PAGE_WIDTH: usize = TILES_PER_ROW * TILE_WIDTH; // 7168
                                        // const PAGE_HEIGHT: usize = TILE_ROWS_PER_PAGE * TILE_HEIGHT; // 135
                                        const TILE_SIZE: usize = TILE_WIDTH * TILE_HEIGHT;

                                        let mut tiles_remaining = file.size / TILE_SIZE;

                                        let mut page_index: usize = 0;

                                        while tiles_remaining > 0 {
                                            // Determine how many tiles per row for this page
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
                                            let tiles_for_page: usize =
                                                row_tile_counts.iter().sum();

                                            // We read per row to keep memory usage modest; build page buffer
                                            let mut page_buffer =
                                                vec![
                                                    0u8;
                                                    PAGE_WIDTH * (rows_this_page * TILE_HEIGHT)
                                                ];

                                            // To reconstruct, we need to read the tiles for each row
                                            let mut tiles_read_in_page = 0usize;
                                            for row in 0..rows_this_page {
                                                let remaining_for_page =
                                                    tiles_for_page - tiles_read_in_page;
                                                let row_tiles =
                                                    row_tile_counts[row].min(remaining_for_page);

                                                // Read row_tiles worth of tile buffers
                                                let tile_data: Vec<[u8; TILE_SIZE]> = (0
                                                    ..row_tiles)
                                                    .map(|_| {
                                                        let mut buf = [0u8; TILE_SIZE];
                                                        dat_buffer.read_exact(&mut buf).unwrap();
                                                        buf
                                                    })
                                                    .collect();

                                                // Write out scanlines for this row into page_buffer
                                                for y in 0..TILE_HEIGHT {
                                                    let dest_row_start =
                                                        (row * TILE_HEIGHT + y) * PAGE_WIDTH;
                                                    let mut dest_offset = dest_row_start;

                                                    // Copy each tile's scanline
                                                    for tile_buf in &tile_data {
                                                        let src_start = y * TILE_WIDTH;
                                                        let src_end = src_start + TILE_WIDTH;
                                                        page_buffer
                                                            [dest_offset..dest_offset + TILE_WIDTH]
                                                            .copy_from_slice(
                                                                &tile_buf[src_start..src_end],
                                                            );
                                                        dest_offset += TILE_WIDTH;
                                                    }

                                                    // Pad remaining tiles in the row with zeros if fewer than TILES_PER_ROW
                                                    let remaining_tiles = TILES_PER_ROW - row_tiles;
                                                    if remaining_tiles > 0 {
                                                        let pad_bytes =
                                                            remaining_tiles * TILE_WIDTH;
                                                        // dest_offset already points to padding start
                                                        for b in &mut page_buffer
                                                            [dest_offset..dest_offset + pad_bytes]
                                                        {
                                                            *b = 0;
                                                        }
                                                    }
                                                }

                                                tiles_read_in_page += row_tiles;
                                            }

                                            // Write KTX2 for this page
                                            let page_pixel_height = rows_this_page * TILE_HEIGHT;
                                            let ktx_header = ktx2::get_ktx2_header(
                                                PAGE_WIDTH as u32,
                                                page_pixel_height as u32,
                                                ktx2::VK_FORMAT_R8_UNORM,
                                                (PAGE_WIDTH * page_pixel_height) as u64,
                                            )?;

                                            let page_name =
                                                format!("{}_{:03}.ktx2", tilea_name, page_index);
                                            let entry = SimpleDataEntry::new(
                                                &mut Cursor::new(&ktx_header)
                                                    .chain(Cursor::new(&page_buffer)),
                                                &dat_path.join(page_name),
                                                arx_creator.adder(),
                                            )?;
                                            arx_creator.add_entry(&entry)?;

                                            page_index += 1;
                                        }
                                    } else if file.name.ends_with(".hpf") {
                                        let mut file_buffer = vec![0u8; file.size];
                                        dat_buffer.read_exact(&mut file_buffer)?;

                                        let signature = u32::from_le_bytes(
                                            file_buffer[0..4].try_into().unwrap(),
                                        );
                                        let mut buf = if signature != 0xFF02AA55 {
                                            &file_buffer[8..]
                                        } else {
                                            &formats::hpf::decompress(&file_buffer)[8..]
                                        };
                                        let hpf_ktx2 = ktx2::get_ktx2_header(
                                            28,
                                            buf.len() as u32 / 28,
                                            ktx2::VK_FORMAT_R8_UNORM,
                                            buf.len() as _,
                                        )?;

                                        let entry = SimpleDataEntry::new(
                                            &mut Cursor::new(&hpf_ktx2)
                                                .chain(Cursor::new(&mut buf)),
                                            &dat_path.join(file.name.replace(".hpf", ".ktx2")),
                                            arx_creator.adder(),
                                        )?;
                                        arx_creator.add_entry(&entry)?;
                                    } else if file.name.ends_with(".mpf") {
                                        let mut file_buffer = vec![0u8; file.size];
                                        dat_buffer.read_exact(&mut file_buffer)?;

                                        let mut reader = Cursor::new(file_buffer);
                                        let mpf = MpfFile::read_from_da(&mut reader)
                                            .expect("Failed to read MPF file");

                                        let mpf_bytes = bincode::encode_to_vec(
                                            mpf,
                                            bincode::config::standard(),
                                        )?;
                                        let entry = SimpleDataEntry::new(
                                            &mut Cursor::new(mpf_bytes),
                                            &dat_path.join(file.name.replace(".mpf", ".mpf.bin")),
                                            arx_creator.adder(),
                                        )?;
                                        arx_creator.add_entry(&entry)?;
                                    } else if file.name.ends_with(".efa") {
                                        let mut file_buffer = vec![0u8; file.size];
                                        dat_buffer.read_exact(&mut file_buffer)?;

                                        let mut reader = Cursor::new(file_buffer);
                                        match EfaFile::read_from_da(&mut reader) {
                                            Ok(efa) => {
                                                let efa_bytes = bincode::encode_to_vec(
                                                    efa,
                                                    bincode::config::standard(),
                                                )?;
                                                let entry = SimpleDataEntry::new(
                                                    &mut Cursor::new(efa_bytes),
                                                    &dat_path.join(
                                                        file.name.replace(".efa", ".efa.bin"),
                                                    ),
                                                    arx_creator.adder(),
                                                )?;
                                                arx_creator.add_entry(&entry)?;
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    "Failed to read EFA file {}: {:?}",
                                                    file.name,
                                                    e
                                                );
                                            }
                                        }
                                    } else if file.name.ends_with(".epf") {
                                        let mut file_buffer = vec![0u8; file.size];
                                        dat_buffer.read_exact(&mut file_buffer)?;

                                        let (
                                            frame_count,
                                            pixel_width,
                                            pixel_height,
                                            _,
                                            toc_address,
                                        ) = {
                                            let mut cursor = Cursor::new(&file_buffer);

                                            (
                                                cursor.read_u16::<LE>()? as usize,
                                                cursor.read_u16::<LE>()? as usize,
                                                cursor.read_u16::<LE>()? as usize,
                                                cursor.read_u16::<LE>()?,
                                                cursor.read_u32::<LE>()? as usize,
                                            )
                                        };

                                        let file_buffer = &file_buffer[12..];

                                        let mut frames = Vec::with_capacity(frame_count);

                                        for i in 0..frame_count {
                                            let (
                                                top,
                                                left,
                                                bottom,
                                                right,
                                                start_address,
                                                _end_address,
                                            ) = {
                                                let mut cursor = Cursor::new(
                                                    &file_buffer[(toc_address + i * 16)..],
                                                );

                                                (
                                                    cursor.read_u16::<LE>()? as usize,
                                                    cursor.read_u16::<LE>()? as usize,
                                                    cursor.read_u16::<LE>()? as usize,
                                                    cursor.read_u16::<LE>()? as usize,
                                                    cursor.read_u32::<LE>()? as usize,
                                                    cursor.read_u32::<LE>()? as usize,
                                                )
                                            };

                                            let width = right - left;
                                            let height = bottom - top;

                                            let bytes_to_read = width * height;
                                            let bytes_available = file_buffer.len() - start_address;

                                            if width == 0
                                                || height == 0
                                                || bytes_to_read > bytes_available
                                            {
                                                frames.push(EpfFrame::new(0, 0, 0, 0, vec![]));
                                                continue;
                                            }

                                            let data = file_buffer
                                                [start_address..(start_address + bytes_to_read)]
                                                .to_vec();

                                            frames.push(EpfFrame::new(
                                                top, left, bottom, right, data,
                                            ));
                                        }

                                        let epf = EpfImage {
                                            width: pixel_width,
                                            height: pixel_height,
                                            frames,
                                        };

                                        if (dat_name.starts_with("khan")
                                            || (dat_name == "Legend"
                                                && file.name.starts_with("emot")))
                                            && file.name != "mf03423.epf"
                                        {
                                            epfs_to_concat.push((file.name.clone(), epf));
                                            continue;
                                        }

                                        let epf_bytes = bincode::encode_to_vec(
                                            epf,
                                            bincode::config::standard(),
                                        )?;

                                        let entry = SimpleDataEntry::new(
                                            &mut Cursor::new(epf_bytes),
                                            &dat_path.join(file.name.replace(".epf", ".epf.bin")),
                                            arx_creator.adder(),
                                        )?;
                                        arx_creator.add_entry(&entry)?;
                                    } else if file.name.ends_with(".spf") {
                                        let mut file_buffer = vec![0u8; file.size];
                                        dat_buffer.read_exact(&mut file_buffer)?;

                                        let mut reader = Cursor::new(file_buffer);
                                        match SpfFile::read_from_da(&mut reader) {
                                            Ok(spf) => {
                                                let base_name = file.name.trim_end_matches(".spf");

                                                for (frame_idx, frame) in
                                                    spf.frames.iter().enumerate()
                                                {
                                                    if frame.width == 0 || frame.height == 0 {
                                                        continue;
                                                    }

                                                    let ktx_header = ktx2::get_ktx2_header(
                                                        frame.width,
                                                        frame.height,
                                                        ktx2::VK_FORMAT_R8G8B8A8_UNORM,
                                                        frame.data.len() as u64,
                                                    )?;

                                                    let frame_name =
                                                        format!("{}.{}.ktx2", base_name, frame_idx);
                                                    let entry = SimpleDataEntry::new(
                                                        &mut Cursor::new(&ktx_header)
                                                            .chain(Cursor::new(&frame.data)),
                                                        &dat_path.join(&frame_name),
                                                        arx_creator.adder(),
                                                    )?;
                                                    arx_creator.add_entry(&entry)?;
                                                }
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    "Failed to read SPF file {}: {:?}",
                                                    file.name,
                                                    e
                                                );
                                            }
                                        }
                                    } else if dat_name == "Legend" && file.name == "color0.tbl" {
                                        let mut file_buffer = vec![0u8; file.size];
                                        dat_buffer.read_exact(&mut file_buffer)?;

                                        let all_lines = String::from_utf8_lossy(&file_buffer);

                                        let lines: Vec<_> = all_lines
                                            .lines()
                                            .filter(|line| !line.is_empty())
                                            .collect();

                                        let colors_per_palette =
                                            lines[0].parse::<u8>().unwrap_or(0) as usize;
                                        let bytes_per_dye = colors_per_palette * 4;
                                        let dye_offset_start = 98;

                                        let lines: Vec<_> = lines.iter().skip(1).collect();

                                        let mut buf = vec![0u8; 256 * 256 * 4];

                                        for lines in lines.chunks_exact(colors_per_palette + 1) {
                                            let i = lines[0].parse::<u8>().unwrap() as usize;

                                            let dye_colors = lines
                                                .iter()
                                                .skip(1)
                                                .take(colors_per_palette)
                                                .flat_map(|c| {
                                                    let mut color = c
                                                        .split(',')
                                                        .map(|c| c.parse::<u8>().unwrap_or(0))
                                                        .collect::<Vec<_>>();

                                                    color.resize(3, 0);
                                                    color.push(255);
                                                    color
                                                })
                                                .collect::<Vec<_>>();

                                            assert_eq!(dye_colors.len(), bytes_per_dye);

                                            let start = i * 256 * 4 + dye_offset_start * 4;
                                            let end = start + bytes_per_dye;

                                            buf[start..end].copy_from_slice(&dye_colors);
                                        }

                                        let tbl_header = ktx2::get_ktx2_header(
                                            256,
                                            256,
                                            ktx2::VK_FORMAT_R8G8B8A8_UNORM,
                                            buf.len() as _,
                                        )?;

                                        let entry = SimpleDataEntry::new(
                                            &mut Cursor::new(&tbl_header).chain(Cursor::new(buf)),
                                            &dat_path.join("color0.ktx2"),
                                            arx_creator.adder(),
                                        )?;
                                        arx_creator.add_entry(&entry)?;
                                    } else {
                                        let mut file_buffer = vec![0u8; file.size];
                                        dat_buffer.read_exact(&mut file_buffer)?;

                                        if file.name.ends_with(".tbl")
                                            || file.name.ends_with(".pal")
                                        {
                                            files_to_process.push((file.name, file_buffer.clone()));
                                        } else {
                                            let entry = SimpleDataEntry::new(
                                                &mut Cursor::new(file_buffer),
                                                &dat_path.join(file.name),
                                                arx_creator.adder(),
                                            )?;
                                            arx_creator.add_entry(&entry)?;
                                        }
                                    }
                                }

                                if files_in_dat.is_empty() {
                                    break;
                                }
                            }
                        }

                        for (palette_dat, palette_name) in [
                            ("seo", "mpt"),
                            ("ia", "stc"),
                            ("ia", "sts"),
                            ("khanpal", "palb"),
                            ("khanpal", "palc"),
                            ("khanpal", "pale"),
                            ("khanpal", "palf"),
                            ("khanpal", "palh"),
                            ("khanpal", "pali"),
                            ("khanpal", "pall"),
                            ("khanpal", "palm"),
                            ("khanpal", "palp"),
                            ("khanpal", "palu"),
                            ("khanpal", "palw"),
                            ("hades", "mns"),
                            ("setoa", "gui"),
                            ("Legend", "item"),
                            ("roh", "eff"),
                        ] {
                            if dat_name != palette_dat {
                                continue;
                            }

                            println!("Processing palette: {}", palette_name);

                            {
                                let buf: Vec<u8> = files_to_process
                                    .iter()
                                    .filter(|(file_name, _)| {
                                        file_name.starts_with(palette_name)
                                            && file_name.ends_with(".tbl")
                                            && !file_name.contains("ani.tbl")
                                            && !file_name.contains("attr.tbl")
                                            && !file_name.contains("effect.tbl")
                                            // && !file_name.contains("palm") != "khanpal"
                                            && dat_name != "hades"
                                    })
                                    .flat_map(|(_, buf)| buf.clone())
                                    .collect();
                                let all_lines = String::from_utf8_lossy(&buf);
                                let lines = all_lines
                                    .split("\r\n")
                                    .into_iter()
                                    .filter(|line| !line.is_empty());

                                let (lines, override_lines): (Vec<_>, Vec<_>) =
                                    lines.partition(|line| {
                                        !(line.ends_with(" -1") || line.ends_with(" -2"))
                                    });

                                let (male_lines, female_lines): (Vec<_>, Vec<_>) = override_lines
                                    .into_iter()
                                    .partition(|line| line.ends_with(" -1"));

                                for (lines, suffix) in
                                    [(lines, ""), (male_lines, "_m"), (female_lines, "_f")]
                                {
                                    let tree: RangeMap<u16, u16> = lines
                                        .iter()
                                        .map(|line| {
                                            let mut parts = line
                                                .trim_end_matches(" -1")
                                                .trim_end_matches(" -2")
                                                .split_ascii_whitespace();

                                            let start =
                                                parts.next().unwrap().parse::<u16>().unwrap();
                                            let end_or_id =
                                                parts.next().unwrap().parse::<u16>().unwrap();

                                            match parts.next() {
                                                Some(id) => {
                                                    let id = id.parse::<u16>().unwrap();
                                                    (start..(end_or_id + 1), id)
                                                }
                                                None => (start..(start + 1), end_or_id),
                                            }
                                        })
                                        .collect();

                                    if !tree.is_empty() {
                                        let tbl = bincode::serde::encode_to_vec(
                                            &tree,
                                            bincode::config::standard(),
                                        )?;

                                        let entry = SimpleDataEntry::new(
                                            &mut Cursor::new(tbl),
                                            &dat_path.join(format!(
                                                "{}{}.tbl.bin",
                                                palette_name, suffix
                                            )),
                                            arx_creator.adder(),
                                        )?;
                                        arx_creator.add_entry(&entry)?;
                                    }
                                }
                            }

                            // Build super palette
                            println!("Building super palette for {}", palette_name);
                            let mut buf: Vec<u8> = files_to_process
                                .iter()
                                .filter(|(file_name, buf)| {
                                    // println!("Processing file: {}", file_name);

                                    file_name.starts_with(palette_name)
                                        && file_name.ends_with(".pal")
                                        && !buf.is_empty()
                                })
                                .flat_map(|(_, buf)| {
                                    let mut target_buf: Vec<u8> = Vec::with_capacity(256 * 4);

                                    for color in buf.chunks_exact(3) {
                                        target_buf.extend_from_slice(&color);
                                        target_buf.push(255);
                                    }

                                    target_buf.resize(256 * 4, 0);

                                    target_buf
                                })
                                .collect();

                            if buf.is_empty() {
                                continue;
                            }

                            const REQUIRED_SIZE: usize = 256 * 256 * 4;
                            if buf.len() < REQUIRED_SIZE {
                                buf.resize(REQUIRED_SIZE, 0);
                            }

                            let tbl_header = ktx2::get_ktx2_header(
                                256,
                                256,
                                ktx2::VK_FORMAT_R8G8B8A8_UNORM,
                                buf.len() as _,
                            )?;

                            let entry = SimpleDataEntry::new(
                                &mut Cursor::new(&tbl_header).chain(Cursor::new(&mut buf)),
                                &dat_path.join(format!("{}.ktx2", palette_name)),
                                arx_creator.adder(),
                            )?;
                            arx_creator.add_entry(&entry)?;
                        }

                        if dat_name != "khanpal" {
                            for (file_name, buf) in files_to_process {
                                let entry = SimpleDataEntry::new(
                                    &mut Cursor::new(buf),
                                    &dat_path.join(file_name),
                                    arx_creator.adder(),
                                )?;
                                arx_creator.add_entry(&entry)?;
                            }
                        }

                        if !epfs_to_concat.is_empty() {
                            // group the files by the first 2 letters of the name
                            let mut epfs_by_prefix: std::collections::HashMap<
                                String,
                                Vec<(String, EpfImage)>,
                            > = std::collections::HashMap::new();

                            for (file_name, epf) in epfs_to_concat {
                                let prefix = if file_name.starts_with("emot") {
                                    "em".to_string()
                                } else {
                                    file_name[..2].to_string()
                                };

                                epfs_by_prefix
                                    .entry(prefix)
                                    .or_default()
                                    .push((file_name, epf));
                            }

                            for (prefix, epfs) in epfs_by_prefix {
                                let mut epfs_by_num: std::collections::HashMap<
                                    String,
                                    Vec<(String, EpfImage)>,
                                > = std::collections::HashMap::new();
                                for (file_name, epf) in epfs {
                                    let num = if file_name.starts_with("emot") {
                                        format!("0{}", &file_name[4..6])
                                    } else {
                                        file_name[2..5].to_string()
                                    };
                                    epfs_by_num.entry(num).or_default().push((file_name, epf));
                                }

                                for (num, epfs) in epfs_by_num {
                                    let epf_animations = epfs
                                        .iter()
                                        .flat_map(|(file_name, epf)| {
                                            let suffix = if file_name.starts_with("emot") {
                                                "emot".to_string()
                                            } else {
                                                file_name[5..].replace(".epf", "")
                                            };

                                            epf.into_animation(&suffix)
                                        })
                                        .collect::<Vec<_>>();

                                    let buf = bincode::encode_to_vec(
                                        epf_animations,
                                        bincode::config::standard(),
                                    )?;
                                    let entry = SimpleDataEntry::new(
                                        &mut Cursor::new(buf),
                                        &Path::new(&format!("khan/{}/{}.epfanim", prefix, num)),
                                        arx_creator.adder(),
                                    )?;
                                    arx_creator.add_entry(&entry)?;
                                }
                            }
                        }
                    } else if is_music {
                        let mut buf = vec![];
                        decoder.read_to_end(&mut buf)?;

                        hasher.update(&buf);

                        let entry = SimpleDataEntry::new(
                            &mut Cursor::new(buf),
                            &Path::new(&file_header.file_path),
                            arx_creator.adder(),
                        )?;
                        arx_creator.add_entry(&entry)?;
                    }

                    // Advance to the end of the file if not already there
                    io::copy(&mut file_reader, &mut io::sink())?;
                    exe_reader = file_reader.into_inner();
                }

                let hash = hasher.finalize();
                assert_eq!(
                    file_header.crc32, hash,
                    "CRC32 mismatch for {}",
                    file_header.file_path
                );

                // read the crc32 of the file
                exe_reader.read_exact(&mut crc32_buffer)?;
                let crc32 = u32::from_le_bytes([
                    crc32_buffer[0],
                    crc32_buffer[1],
                    crc32_buffer[2],
                    crc32_buffer[3],
                ]);
                exe_reader_position = exe_reader_position + file_size + 4;

                assert_eq!(crc32, hash, "CRC32 mismatch for {}", file_header.file_path);
            }
            processed_compressed_size += file_size;
        }
    }

    if let Some(p) = &progress {
        p.report(0.95, "Finalizing archive...".to_string());
    }

    let entry = SimpleDataEntry::new(
        &mut Cursor::new(&VERSION_BUF),
        Path::new("VERSION"),
        arx_creator.adder(),
    )?;
    arx_creator.add_entry(&entry)?;
    if let Some(p) = &progress {
        p.report(0.98, "Writing indexes...".to_string());
    }
    arx_creator.finalize()?;

    if let Some(p) = &progress {
        p.report(1.0, "Installation complete".to_string());
    }

    Ok(())
}

#[derive(Debug)]
struct DatFileEntry {
    name: String,
    size: usize,
}

fn read_header<R: Read + ?Sized + BufRead>(reader: &mut R) {
    reader.seek_relative(43).unwrap();
}

fn read_languages<R: Read + ?Sized + BufRead>(reader: &mut R) -> Result<(), std::io::Error> {
    reader.skip_until(0)?;
    reader.skip_until(0)?;
    reader.skip_until(0)?;

    reader.seek_relative(6)?;

    assert_eq!(reader.read_u8()?, 0x01);

    reader.seek_relative(7)?;

    for _ in 0..56 {
        reader.skip_until(0)?;
    }

    Ok(())
}

fn read_operation<R: Read + ?Sized + BufRead>(reader: &mut R) -> Result<Operation, std::io::Error> {
    let id = reader.read_u8()?;
    match id {
        0x00 => {
            let file_header = read_file_header(reader)?;
            Ok(Operation::CreateFile(file_header))
        }
        0x03 => {
            reader.seek_relative(1)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x04 => {
            reader.seek_relative(1)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x05 => {
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x07 => {
            reader.seek_relative(1)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x08 => {
            reader.seek_relative(1)?;
            Ok(Operation::NoOp)
        }
        0x09 => {
            reader.seek_relative(1)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x0a => {
            reader.seek_relative(2)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x0b => {
            reader.seek_relative(1)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x0c => {
            reader.seek_relative(1)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x0d => Ok(Operation::NoOp),
        0x0f => Ok(Operation::NoOp),
        0x10 => Ok(Operation::NoOp),
        0x11 => {
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x14 => {
            reader.seek_relative(4).unwrap();
            let deflate_end = reader.read_u32::<LE>().unwrap();
            reader.seek_relative(4).unwrap();
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            Ok(Operation::UnknownFile(deflate_end))
        }
        0x15 => {
            reader.seek_relative(1)?;
            reader.skip_until(0)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x16 => {
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x18 => {
            let test_char = reader.read_u8()?;
            assert!(test_char == 0x1b);
            Ok(Operation::NoOp)
        }
        0x1b => Ok(Operation::NoOp),
        0x1c => {
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        0x1e => {
            reader.seek_relative(1)?;
            reader.skip_until(0)?;
            Ok(Operation::NoOp)
        }
        _ => panic!("Unknown operation: 0x{:02X}", id),
    }
}

#[derive(Debug)]
enum Operation {
    NoOp,
    CreateFile(FileHeader),
    UnknownFile(u32),
}

#[derive(Debug)]
struct FileHeader {
    deflate_start: u32,
    deflate_end: u32,
    crc32: u32,
    file_path: String,
}

fn read_file_header<R: Read + ?Sized + BufRead>(
    reader: &mut R,
) -> Result<FileHeader, std::io::Error> {
    reader.seek_relative(2)?;
    let deflate_start = reader.read_u32::<LE>()?;
    let deflate_end = reader.read_u32::<LE>()?;
    reader.seek_relative(28)?;
    let crc32 = reader.read_u32::<LE>()?;
    let file_path = read_null_terminated_string(reader)
        .replace("\\", "/")
        .replace("%MAINDIR%/", "");
    reader.skip_until(0)?;
    reader.skip_until(0)?;

    Ok(FileHeader {
        deflate_start,
        deflate_end,
        crc32,
        file_path,
    })
}

fn read_null_terminated_string<R: Read + ?Sized + BufRead>(reader: &mut R) -> String {
    let mut buffer = Vec::new();
    reader.read_until(0, &mut buffer).unwrap();
    String::from_utf8_lossy(&buffer[..buffer.len() - 1]).to_string()
}

#[derive(Debug)]
struct WiseOverlayHeader {
    wise_script_uncompressed_size: u32,
    eof: u32,
    dib_compressed_size: u32,
}

fn read_wise_overlay_header<R: Read + ?Sized + BufRead>(reader: &mut R) -> WiseOverlayHeader {
    assert_eq!(reader.read_u8().unwrap(), 0);
    reader.seek_relative(24).unwrap();
    let wise_script_uncompressed_size = reader.read_u32::<LE>().unwrap();
    reader.seek_relative(48).unwrap();
    let eof = reader.read_u32::<LE>().unwrap();
    let dib_compressed_size = reader.read_u32::<LE>().unwrap();
    reader.seek_relative(6).unwrap();
    let init_text_length = reader.read_u8().unwrap();
    reader.seek_relative(init_text_length.into()).unwrap();

    WiseOverlayHeader {
        wise_script_uncompressed_size,
        eof,
        dib_compressed_size,
    }
}

trait SeekExt {
    fn seek_relative(&mut self, offset: i64) -> io::Result<()>;
}

impl<T: Read + ?Sized> SeekExt for T {
    fn seek_relative(&mut self, offset: i64) -> io::Result<()> {
        let offset = u64::try_from(offset)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "offset is negative"))?;

        io::copy(&mut self.take(offset), &mut io::sink())?;

        Ok(())
    }
}

struct SimpleDataEntry {
    path: arx::PathBuf,
    kind: arx::create::EntryKind,
}

impl SimpleDataEntry {
    fn new<R: Read>(
        reader: &mut R,
        path: &Path,
        adder: &mut impl ContentAdder,
    ) -> anyhow::Result<Self> {
        let mut data = vec![];
        reader.read_to_end(&mut data)?;

        let size = jbk::Size::new(data.len() as _);
        let content_address =
            adder.add_content(Box::new(Cursor::new(data)), jbk::creator::CompHint::Detect)?;
        Ok(Self {
            path: arx::PathBuf::from_path(path).unwrap(),
            kind: arx::create::EntryKind::File(size, content_address),
        })
    }
}

// Keeping it simple as we don't need timestamp and permissions
impl arx::create::EntryTrait for SimpleDataEntry {
    fn kind(&self) -> Result<Option<arx::create::EntryKind>, CreatorError> {
        Ok(Some(self.kind.clone()))
    }

    fn path(&self) -> &arx::Path {
        &self.path
    }

    fn uid(&self) -> u64 {
        1000
    }

    fn gid(&self) -> u64 {
        1000
    }

    fn mode(&self) -> u64 {
        755
    }

    fn mtime(&self) -> u64 {
        0
    }
}

enum ExeReader {
    File(std::fs::File),
    Http(reqwest::blocking::Response),
}

impl Read for ExeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            ExeReader::File(file) => file.read(buf),
            ExeReader::Http(res) => res.read(buf),
        }
    }
}
