use formats::epf::EpfImage;
use formats::util::parallel_indexed;
use rendering::scene::players::PlayerPieceType;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::asset_record::AssetRecord;
use crate::deferred_job::AnimationAssetJob;
use crate::sheet_processor;

const KHAN_CLEANUP_COPIES: [KhanCleanupEntry; 5] = [
    KhanCleanupEntry::new(PlayerPieceType::Weapon, "130"),
    KhanCleanupEntry::new(PlayerPieceType::Weapon, "131"),
    KhanCleanupEntry::new(PlayerPieceType::HelmetExtra, "103"),
    KhanCleanupEntry::new(PlayerPieceType::HelmetBg, "103"),
    KhanCleanupEntry::new(PlayerPieceType::HelmetFg, "103"),
];

pub(crate) struct AnimationProcessor {
    khan_overrides: HashMap<String, Vec<(String, EpfImage)>>,
    khanf_to_ignore: HashSet<String>,
}

impl AnimationProcessor {
    pub(crate) fn new() -> Self {
        Self {
            khan_overrides: HashMap::new(),
            khanf_to_ignore: KHAN_CLEANUP_COPIES
                .iter()
                .map(|entry| format!("w{}{}", entry.prefix, entry.sprite_num))
                .collect::<HashSet<_>>(),
        }
    }

    pub(crate) fn should_group_epf(&self, dat_name: &str, file_name: &str) -> bool {
        (dat_name.starts_with("khan") || (dat_name == "Legend" && file_name.starts_with("emot")))
            && file_name != "mf03423.epf"
            && !self.khanf_to_ignore.contains(&file_name[..5])
    }

    pub(crate) fn emit_grouped_epfs(
        &mut self,
        job: AnimationAssetJob,
    ) -> anyhow::Result<Vec<AssetRecord>> {
        let AnimationAssetJob {
            dat_name,
            epfs_to_concat,
        } = job;
        if epfs_to_concat.is_empty() {
            return Ok(Vec::new());
        }

        let mut epfs_by_prefix: HashMap<String, Vec<(String, EpfImage)>> = HashMap::new();
        let mut epfs_to_concat = epfs_to_concat;
        let mut records = Vec::new();

        if let Some(overrides) = self.khan_overrides.get(&dat_name) {
            for (name, epf) in overrides {
                epfs_to_concat.push((name.clone(), epf.clone()));
            }
        }

        for (file_name, epf) in epfs_to_concat {
            let prefix = if file_name.starts_with("emot") {
                "em".to_string()
            } else {
                file_name[..2].to_string()
            };

            let existing_epfs = epfs_by_prefix.entry(prefix).or_default();

            if !existing_epfs
                .iter()
                .any(|(existing_name, _)| existing_name == &file_name)
            {
                existing_epfs.push((file_name, epf));
            }
        }

        // Collect every sprite's animation set first, applying the cleanup
        // overrides as we go, then pack the sheets in parallel (in bounded
        // batches) so install time stays low on multi-core machines.
        struct PlayerSheetJob {
            prefix: String,
            num: String,
            animations: Vec<formats::epf::EpfAnimation>,
        }
        let mut sheet_jobs: Vec<PlayerSheetJob> = Vec::new();
        for (prefix, epfs) in epfs_by_prefix {
            let mut epfs_by_num: HashMap<String, Vec<(String, EpfImage)>> = HashMap::new();

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

                        epf.into_animation(&suffix, epf.frames.len())
                    })
                    .collect::<Vec<_>>();

                if prefix.starts_with('m') {
                    for cleanup in KHAN_CLEANUP_COPIES.iter() {
                        let piece_prefix = cleanup.piece_type.prefix(0);
                        let full_prefix = format!("m{}", piece_prefix);
                        if full_prefix == prefix && num == cleanup.sprite_num {
                            let target_dat_name = dat_name.replace("khanm", "khanw");
                            for (file_name, epf) in epfs.iter() {
                                self.khan_overrides
                                    .entry(target_dat_name.clone())
                                    .or_default()
                                    .push((format!("w{}", &file_name[1..]), epf.clone()));
                            }
                        }
                    }
                }

                sheet_jobs.push(PlayerSheetJob {
                    prefix: prefix.clone(),
                    num,
                    animations: epf_animations,
                });
            }
        }

        const BATCH_SIZE: usize = 32;
        let worker_count = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .max(1);
        for batch in sheet_jobs.chunks(BATCH_SIZE) {
            let batch_records =
                parallel_indexed(batch.len(), worker_count.min(batch.len()), |index| {
                    let job = &batch[index];
                    build_player_sheet_records(&job.prefix, &job.num, &job.animations)
                });
            for (_, result) in batch_records {
                records.extend(result?);
            }
        }

        Ok(records)
    }
}

/// Packs one player part's animations into sheet chunks and returns the
/// single per-asset sheet record (oxicode metadata + raw chunk pixels).
fn build_player_sheet_records(
    prefix: &str,
    num: &str,
    animations: &[formats::epf::EpfAnimation],
) -> anyhow::Result<Vec<AssetRecord>> {
    let (sheet, sheet_images) = sheet_processor::build_player_sheets(animations);
    let sheet_bytes = oxicode::encode_to_vec(&sheet)?;
    sheet_processor::sheet_records(
        Path::new("khan"),
        &format!("{prefix}/{num}"),
        1,
        sheet_bytes,
        sheet.chunks,
        sheet_images,
    )
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct KhanCleanupEntry {
    piece_type: PlayerPieceType,
    prefix: char,
    sprite_num: &'static str,
}

impl KhanCleanupEntry {
    const fn new(piece_type: PlayerPieceType, sprite_num: &'static str) -> Self {
        Self {
            piece_type,
            prefix: piece_type.prefix(0),
            sprite_num,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AnimationProcessor, build_player_sheet_records};
    use crate::deferred_job::AnimationAssetJob;
    use formats::epf::{AnimationDirection, EpfAnimation, EpfAnimationType, EpfFrame, EpfImage};
    use std::collections::HashMap;
    use std::io::Read;
    use std::path::Path;

    #[test]
    fn should_group_epf_respects_khan_and_ignore_rules() {
        let processor = AnimationProcessor::new();

        assert!(processor.should_group_epf("khanm1", "ma123a.epf"));
        assert!(processor.should_group_epf("Legend", "emot01.epf"));
        assert!(!processor.should_group_epf("Legend", "mf03423.epf"));
        assert!(!processor.should_group_epf("other", "ma123a.epf"));
        assert!(!processor.should_group_epf("khanm1", "wh103a.epf"));
    }

    fn test_epf(id: u8) -> EpfImage {
        EpfImage {
            width: 32,
            height: 32,
            frames: vec![
                EpfFrame::new(0, 0, 16, 16, vec![id; 16 * 16]),
                EpfFrame::new(0, 0, 0, 0, vec![]),
            ],
        }
    }

    #[test]
    fn emit_grouped_epfs_produces_sheet_records() {
        let mut processor = AnimationProcessor::new();
        let job = AnimationAssetJob {
            dat_name: "khanm1".to_string(),
            epfs_to_concat: vec![
                ("ma00101.epf".to_string(), test_epf(1)),
                ("mb00202.epf".to_string(), test_epf(2)),
            ],
        };

        let records = processor.emit_grouped_epfs(job).unwrap();
        let paths: Vec<String> = records
            .iter()
            .map(|record| record.path().to_string_lossy().into_owned())
            .collect();

        assert!(paths.contains(&"khan/ma/001.sheet.bin".to_string()));
        assert!(paths.contains(&"khan/mb/002.sheet.bin".to_string()));
        assert!(!paths.iter().any(|path| path.ends_with(".ktx2")));
    }

    #[test]
    fn player_sheet_records_are_readable_assets() {
        let animations = vec![EpfAnimation {
            animation_type: EpfAnimationType::Idle,
            direction: AnimationDirection::Away,
            image: test_epf(9),
        }];
        let records = build_player_sheet_records("mm", "042", &animations).unwrap();
        assert_eq!(records.len(), 1);

        let mut paths = HashMap::new();
        for record in records {
            let path = record.path().to_path_buf();
            let mut reader = record.into_reader();
            let mut bytes = Vec::new();
            Read::read_to_end(&mut reader, &mut bytes).unwrap();
            paths.insert(path, bytes);
        }
        assert!(paths.contains_key(Path::new("khan/mm/042.sheet.bin")));
        assert!(paths[Path::new("khan/mm/042.sheet.bin")].len() > 100);
    }
}
