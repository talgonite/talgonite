use std::io::Read;
use std::path::Path;
use tracing::info;

use crate::animation_processor::AnimationProcessor;
use crate::asset_record::AssetRecord;
use crate::da741::{PayloadEntry, PayloadKind};
use crate::dat::{DatEntryMetadata, read_dat_entries};
use crate::deferred_job::{AnimationAssetJobBuilder, DeferredAssetJob, PaletteAssetJobBuilder};
use crate::palette_processor::PaletteProcessor;
use crate::raw_asset_processor::{RawAssetProcessor, RawDatEntry};
use crate::sink::AssetSink;
use crate::structured_format_processor::{StructuredDatEntry, StructuredFormatProcessor};
use crate::texture_processor::TextureAssetProcessor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DatEntryRoute {
    Process,
    Rewrite { dat_name: &'static str },
    Skip,
}

fn route_dat_entry(misc_overrides_enabled: bool, dat_name: &str, file_name: &str) -> DatEntryRoute {
    if !misc_overrides_enabled {
        return DatEntryRoute::Process;
    }

    if dat_name.eq_ignore_ascii_case("misc") {
        return match file_name {
            "mns083.mpf" => DatEntryRoute::Rewrite { dat_name: "hades" },
            "wu16501.epf" => DatEntryRoute::Rewrite {
                dat_name: "khanwtz",
            },
            _ => DatEntryRoute::Skip,
        };
    }

    if dat_name.eq_ignore_ascii_case("hades") && file_name == "mns083.mpf" {
        return DatEntryRoute::Skip;
    }

    if dat_name.eq_ignore_ascii_case("khanwtz") && file_name == "wu16501.epf" {
        return DatEntryRoute::Skip;
    }

    DatEntryRoute::Process
}

pub(crate) struct Da741Profile {
    animation_processor: AnimationProcessor,
    misc_overrides_enabled: bool,
    palette_processor: PaletteProcessor,
    raw_asset_processor: RawAssetProcessor,
    structured_format_processor: StructuredFormatProcessor,
    texture_processor: TextureAssetProcessor,
}

impl Da741Profile {
    pub(crate) fn new(misc_overrides_enabled: bool) -> anyhow::Result<Self> {
        Ok(Self {
            animation_processor: AnimationProcessor::new(),
            misc_overrides_enabled,
            palette_processor: PaletteProcessor,
            raw_asset_processor: RawAssetProcessor,
            structured_format_processor: StructuredFormatProcessor,
            texture_processor: TextureAssetProcessor,
        })
    }

    pub(crate) fn process_payload<S: AssetSink>(
        &mut self,
        payload: &PayloadEntry,
        reader: &mut dyn Read,
        sink: &mut S,
    ) -> anyhow::Result<()> {
        match payload.kind {
            PayloadKind::Dat => self.process_dat(payload, reader, sink),
            PayloadKind::Music => sink.write(
                self.raw_asset_processor
                    .process_payload(Path::new(&payload.file_path), reader)?,
            ),
            PayloadKind::Other => Ok(()),
        }
    }

    fn process_dat<S: AssetSink>(
        &mut self,
        payload: &PayloadEntry,
        decoder: &mut dyn Read,
        sink: &mut S,
    ) -> anyhow::Result<()> {
        let dat_path = payload.file_path.replace(".dat", "");
        let dat_path = Path::new(&dat_path);

        let dat_name = dat_path.file_name().unwrap().to_string_lossy().to_string();
        info!("Extracting dat: {}", dat_name);
        let mut palette_job = PaletteAssetJobBuilder::new(&dat_name, dat_path);
        let mut animation_job = AnimationAssetJobBuilder::new(&dat_name);

        read_dat_entries(decoder, &mut |file, entry_reader| {
            let DatEntryMetadata {
                name: file_name,
                size: file_size,
            } = file;

            match route_dat_entry(self.misc_overrides_enabled, &dat_name, &file_name) {
                DatEntryRoute::Process => self.process_dat_entry(
                    sink,
                    dat_path,
                    &dat_name,
                    file_name,
                    file_size,
                    entry_reader,
                    &mut palette_job,
                    &mut animation_job,
                ),
                DatEntryRoute::Rewrite {
                    dat_name: effective_dat_name,
                } => {
                    info!(
                        "Routing {} from {}.dat through {}.dat context",
                        file_name, dat_name, effective_dat_name
                    );
                    self.process_dat_entry(
                        sink,
                        Path::new(effective_dat_name),
                        effective_dat_name,
                        file_name,
                        file_size,
                        entry_reader,
                        &mut palette_job,
                        &mut animation_job,
                    )
                }
                DatEntryRoute::Skip => {
                    info!("Skipping {} from {}.dat", file_name, dat_name);
                    Ok(())
                }
            }
        })?;

        for deferred_job in [palette_job.finish(), animation_job.finish()]
            .into_iter()
            .flatten()
        {
            let records = self.execute_deferred_job(deferred_job)?;
            self.emit_records(sink, records)?;
        }

        Ok(())
    }

    fn process_dat_entry<S: AssetSink>(
        &mut self,
        sink: &mut S,
        dat_path: &Path,
        dat_name: &str,
        file_name: String,
        file_size: usize,
        entry_reader: &mut dyn Read,
        palette_job: &mut PaletteAssetJobBuilder,
        animation_job: &mut AnimationAssetJobBuilder,
    ) -> anyhow::Result<()> {
        if let Some(records) =
            self.texture_processor
                .try_process(dat_path, &file_name, file_size, entry_reader)?
        {
            self.emit_records(sink, records)?;
            return Ok(());
        }

        match self.structured_format_processor.process_entry(
            dat_path,
            &file_name,
            file_size,
            entry_reader,
            self.animation_processor
                .should_group_epf(dat_name, &file_name),
        )? {
            StructuredDatEntry::Unhandled => {}
            StructuredDatEntry::Assets(records) => {
                self.emit_records(sink, records)?;
                return Ok(());
            }
            StructuredDatEntry::GroupedEpf { file_name, epf } => {
                animation_job.push(file_name, epf);
                return Ok(());
            }
        }

        if let Some(record) = self.palette_processor.process_special_entry(
            dat_name,
            dat_path,
            &file_name,
            file_size,
            entry_reader,
        )? {
            sink.write(record)?;
            return Ok(());
        }

        let mut file_buffer = vec![0u8; file_size];
        entry_reader.read_exact(&mut file_buffer)?;

        match self
            .raw_asset_processor
            .process_dat_entry(dat_path, file_name, file_buffer)
        {
            RawDatEntry::DeferredPalette { name, bytes } => {
                palette_job.push(name, bytes);
            }
            RawDatEntry::Asset(record) => {
                sink.write(record)?;
            }
        }

        Ok(())
    }

    fn execute_deferred_job(
        &mut self,
        deferred_job: DeferredAssetJob,
    ) -> anyhow::Result<Vec<AssetRecord>> {
        match deferred_job {
            DeferredAssetJob::Palette(job) => self.palette_processor.process(job),
            DeferredAssetJob::Animation(job) => self.animation_processor.emit_grouped_epfs(job),
        }
    }

    fn emit_records<S: AssetSink>(
        &self,
        sink: &mut S,
        records: Vec<AssetRecord>,
    ) -> anyhow::Result<()> {
        for record in records {
            sink.write(record)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{DatEntryRoute, route_dat_entry};

    #[test]
    fn route_dat_entry_rewrites_misc_overrides() {
        assert_eq!(
            route_dat_entry(true, "misc", "mns083.mpf"),
            DatEntryRoute::Rewrite { dat_name: "hades" }
        );
        assert_eq!(
            route_dat_entry(true, "misc", "wu16501.epf"),
            DatEntryRoute::Rewrite {
                dat_name: "khanwtz"
            }
        );
    }

    #[test]
    fn route_dat_entry_skips_original_override_sources() {
        assert_eq!(
            route_dat_entry(true, "hades", "mns083.mpf"),
            DatEntryRoute::Skip
        );
        assert_eq!(
            route_dat_entry(true, "khanwtz", "wu16501.epf"),
            DatEntryRoute::Skip
        );
    }

    #[test]
    fn route_dat_entry_skips_misc_extras() {
        assert_eq!(
            route_dat_entry(true, "misc", "other.bin"),
            DatEntryRoute::Skip
        );
    }

    #[test]
    fn route_dat_entry_leaves_other_entries_alone() {
        assert_eq!(
            route_dat_entry(true, "hades", "other.bin"),
            DatEntryRoute::Process
        );
        assert_eq!(
            route_dat_entry(true, "misc2", "mns083.mpf"),
            DatEntryRoute::Process
        );
    }

    #[test]
    fn route_dat_entry_disables_misc_overrides_when_misc_payload_is_missing() {
        assert_eq!(
            route_dat_entry(false, "hades", "mns083.mpf"),
            DatEntryRoute::Process
        );
        assert_eq!(
            route_dat_entry(false, "khanwtz", "wu16501.epf"),
            DatEntryRoute::Process
        );
        assert_eq!(
            route_dat_entry(false, "misc", "mns083.mpf"),
            DatEntryRoute::Process
        );
    }
}
