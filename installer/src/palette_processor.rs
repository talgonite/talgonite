use formats::ktx2;
use rangemap::RangeMap;
use std::io::Read;

use crate::asset_record::AssetRecord;
use crate::deferred_job::PaletteAssetJob;

#[derive(Default)]
pub(crate) struct PaletteProcessor;

impl PaletteProcessor {
    pub(crate) fn process_special_entry(
        &self,
        dat_name: &str,
        dat_path: &std::path::Path,
        file_name: &str,
        file_size: usize,
        entry_reader: &mut dyn Read,
    ) -> anyhow::Result<Option<AssetRecord>> {
        if dat_name != "Legend" || file_name != "color0.tbl" {
            return Ok(None);
        }

        let mut file_buffer = vec![0u8; file_size];
        entry_reader.read_exact(&mut file_buffer)?;

        let all_lines = String::from_utf8_lossy(&file_buffer);
        let lines: Vec<_> = all_lines.lines().filter(|line| !line.is_empty()).collect();

        let colors_per_palette = lines[0].parse::<u8>().unwrap_or(0) as usize;
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
                .flat_map(|color| {
                    let mut color = color
                        .split(',')
                        .map(|component| {
                            if i == 18 && component == "491" {
                                235
                            } else {
                                component.parse::<u8>().unwrap_or(0)
                            }
                        })
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

        let tbl_header =
            ktx2::get_ktx2_header(256, 256, ktx2::VK_FORMAT_R8G8B8A8_UNORM, buf.len() as _)?;

        Ok(Some(AssetRecord::chunks(
            dat_path.join("color0.ktx2"),
            vec![tbl_header, buf],
        )))
    }

    pub(crate) fn process(&self, job: PaletteAssetJob) -> anyhow::Result<Vec<AssetRecord>> {
        let PaletteAssetJob {
            dat_name,
            dat_path,
            files_to_process,
        } = job;
        let dat_path = dat_path.as_path();
        let mut records = Vec::new();

        for palette_name in palette_targets(&dat_name) {
            tracing::info!("Processing palette: {}", palette_name);

            {
                let buf: Vec<u8> = files_to_process
                    .iter()
                    .filter(|(file_name, _)| {
                        file_name.starts_with(palette_name)
                            && file_name.ends_with(".tbl")
                            && !file_name.contains("ani.tbl")
                            && !file_name.contains("attr.tbl")
                            && !file_name.contains("effect.tbl")
                            && dat_name != "hades"
                    })
                    .flat_map(|(_, buf)| buf.clone())
                    .collect();
                let all_lines = String::from_utf8_lossy(&buf);

                if palette_name == &"palc" {
                    tracing::info!("All lines for palc:\n{}", all_lines);
                }

                let lines = all_lines.split("\r\n").filter(|line| !line.is_empty());

                let (lines, override_lines): (Vec<_>, Vec<_>) =
                    lines.partition(|line| !(line.ends_with(" -1") || line.ends_with(" -2")));

                let (male_lines, female_lines): (Vec<_>, Vec<_>) = override_lines
                    .into_iter()
                    .partition(|line| line.ends_with(" -1"));

                for (lines, suffix) in [(lines, ""), (male_lines, "_m"), (female_lines, "_f")] {
                    let tree: RangeMap<u16, u16> = lines
                        .iter()
                        .map(|line| {
                            let mut parts = line
                                .trim_end_matches(" -1")
                                .trim_end_matches(" -2")
                                .split_ascii_whitespace();

                            let start = parts.next().unwrap().parse::<u16>().unwrap();
                            let end_or_id = parts.next().unwrap().parse::<u16>().unwrap();

                            match parts.next() {
                                Some(id) => {
                                    let id = id.parse::<u16>().unwrap();
                                    if palette_name == &"palc" {
                                        tracing::info!(
                                            "Parsed line for palc with ID: start={}, end={}, id={}",
                                            start,
                                            end_or_id,
                                            id,
                                        );
                                    }
                                    (start..(end_or_id + 1), id)
                                }
                                None => {
                                    if palette_name == &"palc" {
                                        tracing::info!(
                                            "Parsed line for palc without ID: entry={}, id={}",
                                            start,
                                            end_or_id,
                                        );
                                    }
                                    (start..(start + 1), end_or_id)
                                }
                            }
                        })
                        .collect();

                    if palette_name == &"palc" {
                        tracing::info!("Constructed RangeMap for palc {:?}", tree);
                    }

                    if !tree.is_empty() {
                        let tbl =
                            bincode::serde::encode_to_vec(&tree, bincode::config::standard())?;
                        records.push(AssetRecord::bytes(
                            dat_path.join(format!("{}{}.tbl.bin", palette_name, suffix)),
                            tbl,
                        ));
                    }
                }
            }

            println!("Building super palette for {}", palette_name);
            let mut buf: Vec<u8> = files_to_process
                .iter()
                .filter(|(file_name, buf)| {
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

            let tbl_header =
                ktx2::get_ktx2_header(256, 256, ktx2::VK_FORMAT_R8G8B8A8_UNORM, buf.len() as _)?;

            records.push(AssetRecord::chunks(
                dat_path.join(format!("{}.ktx2", palette_name)),
                vec![tbl_header, buf],
            ));
        }

        if dat_name != "khanpal" {
            for (file_name, buf) in files_to_process {
                records.push(AssetRecord::bytes(dat_path.join(file_name), buf));
            }
        }

        Ok(records)
    }
}

fn palette_targets(dat_name: &str) -> &'static [&'static str] {
    match dat_name {
        "seo" => &["mpt"],
        "ia" => &["stc", "sts"],
        "khanpal" => &[
            "palb", "palc", "pale", "palf", "palh", "pali", "pall", "palm", "palp", "palu", "palw",
        ],
        "hades" => &["mns"],
        "setoa" => &["gui"],
        "Legend" => &["item"],
        "roh" => &["eff"],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::palette_targets;

    #[test]
    fn palette_targets_match_known_dat_groups() {
        assert_eq!(palette_targets("ia"), &["stc", "sts"]);
        assert_eq!(palette_targets("Legend"), &["item"]);
        assert!(palette_targets("unknown").is_empty());
    }
}
