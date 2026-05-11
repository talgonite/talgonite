use formats::epf::EpfImage;
use rendering::scene::players::PlayerPieceType;
use std::collections::{HashMap, HashSet};

use crate::asset_record::AssetRecord;
use crate::deferred_job::AnimationAssetJob;

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

                let buf = bincode::encode_to_vec(epf_animations, bincode::config::standard())?;

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

                records.push(AssetRecord::bytes(
                    format!("khan/{}/{}.epfanim", prefix, num),
                    buf,
                ));
            }
        }

        Ok(records)
    }
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
    use super::AnimationProcessor;

    #[test]
    fn should_group_epf_respects_khan_and_ignore_rules() {
        let processor = AnimationProcessor::new();

        assert!(processor.should_group_epf("khanm1", "ma123a.epf"));
        assert!(processor.should_group_epf("Legend", "emot01.epf"));
        assert!(!processor.should_group_epf("Legend", "mf03423.epf"));
        assert!(!processor.should_group_epf("other", "ma123a.epf"));
        assert!(!processor.should_group_epf("khanm1", "wh103a.epf"));
    }
}