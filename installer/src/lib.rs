use backhand::InnerNode;
use std::{
    io::{BufReader, Read},
    path::Path,
    sync::Arc,
};
use tracing::info;

mod animation_processor;
mod asset_record;
mod da741;
mod da741_profile;
mod dat;
mod deferred_job;
mod palette_processor;
mod raw_asset_processor;
mod sheet_processor;
mod sink;
mod source;
mod structured_format_processor;
mod texture_processor;

use crate::asset_record::AssetRecord;
use crate::da741::{Da741ExeReader, PayloadKind};
use crate::da741_profile::Da741Profile;
use crate::sink::{AssetSink, SquashfsAssetSink};
use crate::source::InstallSource;

const VERSION_BUF: &[u8] = b"741_8";

pub trait InstallProgress: Send + Sync {
    fn report(&self, percent: f32, message: String);
}

fn open_archive(path: &Path) -> anyhow::Result<backhand::FilesystemReader<'static>> {
    let file = std::fs::File::open(path)?;
    Ok(backhand::FilesystemReader::from_reader(BufReader::new(
        file,
    ))?)
}

pub fn is_archive_up_to_date(path: &Path) -> anyhow::Result<bool> {
    let existing_archive = match open_archive(path) {
        Ok(archive) => archive,
        // Unreadable or old-format archive; the installer will rebuild it.
        Err(_) => return Ok(false),
    };
    let version_file = archive_version_file(&existing_archive)?;

    Ok(version_file.as_deref() == Some(VERSION_BUF))
}

pub fn install(output: &Path, progress: Option<Arc<dyn InstallProgress>>) -> anyhow::Result<()> {
    if let Some(p) = &progress {
        p.report(0.0, "Checking archive...".to_string());
    }
    if output.exists() {
        match open_archive(output) {
            Ok(existing_archive) => {
                let version_file = archive_version_file(&existing_archive)?;
                if version_file.as_deref() == Some(VERSION_BUF) {
                    info!("Archive is up to date");
                    return Ok(());
                }
                info!("Archive is not up to date, updating");
            }
            Err(error) => {
                info!("Existing archive is not readable (old format?): {error}; rebuilding");
            }
        }
    } else {
        info!("Archive does not exist, creating");
    }

    let install_source = InstallSource::for_output(output)?;
    let mut exe_reader = Da741ExeReader::from_source(install_source.open()?)?;
    let mut asset_sink = SquashfsAssetSink::new(output)?;

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

fn archive_version_file(
    existing_archive: &backhand::FilesystemReader,
) -> anyhow::Result<Option<Vec<u8>>> {
    for node in existing_archive.files() {
        // Normalized components so this also matches Windows separators.
        let is_version = node
            .fullpath
            .components()
            .filter(|component| matches!(component, std::path::Component::Normal(_)))
            .eq(Path::new("VERSION").components());
        if is_version {
            if let InnerNode::File(file) = &node.inner {
                let mut reader = existing_archive.file(file).reader();
                let mut buf = vec![];
                reader.read_to_end(&mut buf)?;
                return Ok(Some(buf));
            }
        }
    }
    Ok(None)
}
