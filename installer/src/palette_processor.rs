use formats::ktx2;
use formats::palette::{AnimatedPaletteRange, AnimatedPaletteTable};
use rangemap::RangeMap;
use std::io::Read;
use std::ops::Range;

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

            let is_animated_palette_file =
                |file_name: &str| is_animated_palette_table_file(palette_name, file_name);

            {
                let buf: Vec<u8> = files_to_process
                    .iter()
                    .filter(|(file_name, _)| {
                        file_name.starts_with(palette_name)
                            && file_name.ends_with(".tbl")
                            && !is_animated_palette_file(file_name)
                            && !file_name.contains("ani.tbl")
                            && !file_name.contains("attr.tbl")
                            && !file_name.contains("effect.tbl")
                            && dat_name != "hades"
                    })
                    .flat_map(|(_, buf)| buf.clone())
                    .collect();
                let all_lines = String::from_utf8_lossy(&buf);

                let lines = all_lines.split("\r\n").filter(|line| !line.is_empty());

                let (lines, override_lines): (Vec<_>, Vec<_>) =
                    lines.partition(|line| !(line.ends_with(" -1") || line.ends_with(" -2")));

                let (male_lines, female_lines): (Vec<_>, Vec<_>) = override_lines
                    .into_iter()
                    .partition(|line| line.ends_with(" -1"));

                for (lines, suffix) in [(lines, ""), (male_lines, "_m"), (female_lines, "_f")] {
                    let (range_entries, single_entries): (Vec<_>, Vec<_>) = lines
                        .iter()
                        .map(|line| parse_palette_line(line))
                        .partition(|(range, _)| range.end - range.start > 1);

                    let tree: RangeMap<u16, u16> = range_entries
                        .into_iter()
                        .chain(single_entries.into_iter())
                        .collect();

                    if !tree.is_empty() {
                        let tbl =
                            oxicode::serde::encode_to_vec(&tree, oxicode::config::standard())?;
                        records.push(AssetRecord::bytes(
                            dat_path.join(format!("{}{}.tbl.bin", palette_name, suffix)),
                            tbl,
                        ));
                    }
                }
            }

            // Per-palette animated palette tables (stc0006.tbl, mpt0018.tbl, ...),
            // emitted separately so the per-tile palette table above stays clean.
            let mut animated_entries: Vec<(u16, Vec<AnimatedPaletteRange>)> = files_to_process
                .iter()
                .filter(|(file_name, _)| is_animated_palette_file(file_name))
                .filter_map(|(file_name, buf)| {
                    let palette_number = palette_number_from_file_name(palette_name, file_name)?;
                    let ranges = parse_animated_palette_file(buf);

                    if ranges.is_empty() {
                        None
                    } else {
                        Some((palette_number, ranges))
                    }
                })
                .collect();

            if !animated_entries.is_empty() {
                animated_entries.sort_by_key(|(palette_number, _)| *palette_number);

                let table = AnimatedPaletteTable {
                    entries: animated_entries,
                };
                let tbl = oxicode::encode_to_vec(&table)?;
                records.push(AssetRecord::bytes(
                    dat_path.join(format!("{}.anipal.tbl.bin", palette_name)),
                    tbl,
                ));
            }

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

/// Returns true when the file is a per-palette animated palette table, e.g.
/// `stc0006.tbl` (as opposed to the main `stcpal.tbl` palette table).
fn is_animated_palette_table_file(palette_name: &str, file_name: &str) -> bool {
    let Some(rest) = file_name.strip_prefix(palette_name) else {
        return false;
    };
    let Some(stem) = rest.strip_suffix(".tbl") else {
        return false;
    };

    !stem.is_empty() && stem.chars().all(|c| c.is_ascii_digit())
}

/// Extracts the palette number encoded in an animated palette file name such as
/// `stc0006.tbl` -> 6.
fn palette_number_from_file_name(palette_name: &str, file_name: &str) -> Option<u16> {
    let rest = file_name.strip_prefix(palette_name)?;
    let stem = rest.strip_suffix(".tbl")?;
    stem.parse::<u16>().ok()
}

fn parse_animated_palette_file(buf: &[u8]) -> Vec<AnimatedPaletteRange> {
    let text = String::from_utf8_lossy(buf);
    let mut ranges = Vec::new();

    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() != 3 {
            continue;
        }

        let (Ok(start_index), Ok(end_index), Ok(period)) = (
            parts[0].parse::<u8>(),
            parts[1].parse::<u8>(),
            parts[2].parse::<u16>(),
        ) else {
            continue;
        };

        if end_index < start_index {
            continue;
        }

        ranges.push(AnimatedPaletteRange {
            start_index,
            end_index,
            period,
        });
    }

    ranges
}

fn parse_palette_line(line: &str) -> (Range<u16>, u16) {
    let mut parts = line
        .trim_end_matches(" -1")
        .trim_end_matches(" -2")
        .split_ascii_whitespace();

    let start = parts.next().unwrap().parse::<u16>().unwrap();
    let end_or_id = parts.next().unwrap().parse::<u16>().unwrap();

    match parts.next() {
        Some(id) => {
            let id = id.parse::<u16>().unwrap();
            (start..(end_or_id + 1), id)
        }
        None => (start..(start + 1), end_or_id),
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
    use super::{is_animated_palette_table_file, parse_animated_palette_file};
    use formats::palette::AnimatedPaletteRange;

    #[test]
    fn palette_targets_match_known_dat_groups() {
        assert_eq!(palette_targets("ia"), &["stc", "sts"]);
        assert_eq!(palette_targets("Legend"), &["item"]);
        assert!(palette_targets("unknown").is_empty());
    }

    #[test]
    fn animated_palette_files_are_identified_by_numeric_suffix() {
        assert!(is_animated_palette_table_file("stc", "stc0006.tbl"));
        assert!(is_animated_palette_table_file("mpt", "mpt0018.tbl"));
        assert!(!is_animated_palette_table_file("stc", "stcpal.tbl"));
        assert!(!is_animated_palette_table_file("stc", "stcani.tbl"));
        assert!(!is_animated_palette_table_file("stc", "stc.tbl"));
    }

    #[test]
    fn animated_palette_file_lines_parse_to_ranges() {
        let ranges = parse_animated_palette_file(b"236 241 2\r\n214 219 3\n220 225 3\n");

        assert_eq!(
            ranges,
            vec![
                AnimatedPaletteRange {
                    start_index: 236,
                    end_index: 241,
                    period: 2,
                },
                AnimatedPaletteRange {
                    start_index: 214,
                    end_index: 219,
                    period: 3,
                },
                AnimatedPaletteRange {
                    start_index: 220,
                    end_index: 225,
                    period: 3,
                },
            ]
        );
    }

    #[test]
    fn animated_palette_table_round_trips_through_oxicode() {
        let table = formats::palette::AnimatedPaletteTable {
            entries: vec![(
                6,
                vec![AnimatedPaletteRange {
                    start_index: 236,
                    end_index: 241,
                    period: 2,
                }],
            )],
        };

        let encoded = oxicode::encode_to_vec(&table).unwrap();
        let (decoded, _): (formats::palette::AnimatedPaletteTable, usize) =
            oxicode::decode_from_slice(&encoded).unwrap();

        assert_eq!(decoded, table);
    }
}
