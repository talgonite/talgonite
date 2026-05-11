use jubako::{self as jbk};
use libarx::{self as arx, FullBuilder};
use std::{io::Read, path::Path, sync::Arc};
use tracing::info;

mod animation_processor;
mod asset_record;
mod da741;
mod da741_profile;
mod dat;
mod deferred_job;
mod palette_processor;
mod raw_asset_processor;
mod sink;
mod source;
mod structured_format_processor;
mod texture_processor;

use crate::asset_record::AssetRecord;
use crate::da741::{Da741ExeReader, PayloadKind};
use crate::da741_profile::Da741Profile;
use crate::sink::{ArxAssetSink, AssetSink};
use crate::source::InstallSource;

const VERSION_BUF: &[u8] = b"741_3";

pub trait InstallProgress: Send + Sync {
    fn report(&self, percent: f32, message: String);
}

pub fn is_archive_up_to_date(path: &Path) -> anyhow::Result<bool> {
    let existing_archive = libarx::Arx::new(path)?;
    let version_file = archive_version_file(&existing_archive)?;

    Ok(version_file.as_deref() == Some(VERSION_BUF))
}

pub fn install(output: &Path, progress: Option<Arc<dyn InstallProgress>>) -> anyhow::Result<()> {
    if let Some(p) = &progress {
        p.report(0.0, "Checking archive...".to_string());
    }
    if output.exists() {
        let existing_archive = libarx::Arx::new(output).unwrap();

        let version_file = archive_version_file(&existing_archive).unwrap();

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

    let install_source = InstallSource::for_output(output)?;
    let mut exe_reader = Da741ExeReader::from_source(install_source.open()?)?;
    let mut asset_sink = ArxAssetSink::new(output)?;

    let payloads = exe_reader
        .payloads()
        .iter()
        .filter(|payload| matches!(payload.kind, PayloadKind::Dat | PayloadKind::Music))
        .cloned()
        .collect::<Vec<_>>();
    let has_misc_dat = payloads.iter().any(|payload| {
        matches!(payload.kind, PayloadKind::Dat)
            && Path::new(&payload.file_path)
                .file_stem()
                .map(|file_stem| file_stem.to_string_lossy().eq_ignore_ascii_case("misc"))
                .unwrap_or(false)
    });

    let mut profile = Da741Profile::new(has_misc_dat)?;

    let total_compressed_size: u64 = payloads.iter().map(|payload| payload.compressed_size).sum();

    let mut processed_compressed_size: u64 = 0;
    for payload in payloads {
        let file_size = payload.compressed_size;

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
                    payload.file_path,
                    extract_p * 100.0
                ),
            );
        }

        exe_reader.read_payload(&payload, |decoder| {
            profile.process_payload(&payload, decoder, &mut asset_sink)
        })?;

        processed_compressed_size += file_size;
    }

    if let Some(p) = &progress {
        p.report(0.95, "Finalizing archive...".to_string());
    }

    asset_sink.write(AssetRecord::bytes(
        Path::new("VERSION"),
        VERSION_BUF.to_vec(),
    ))?;
    if let Some(p) = &progress {
        p.report(0.98, "Writing indexes...".to_string());
    }
    asset_sink.finalize()?;

    if let Some(p) = &progress {
        p.report(1.0, "Installation complete".to_string());
    }

    Ok(())
}

fn archive_version_file(existing_archive: &libarx::Arx) -> anyhow::Result<Option<Vec<u8>>> {
    let version_file = match existing_archive.get_entry::<FullBuilder>(arx::Path::new("VERSION")) {
        Ok(arx::Entry::File(content_address)) => {
            match existing_archive.get_bytes(content_address.content())? {
                Some(jbk::reader::MayMissPack::FOUND(Some(bytes))) => {
                    let mut buf = vec![];
                    bytes.stream().read_to_end(&mut buf)?;
                    Some(buf)
                }
                _ => None,
            }
        }
        _ => None,
    };

    Ok(version_file)
}
