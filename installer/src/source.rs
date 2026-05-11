use anyhow::Context;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use tracing::debug;

const INSTALLER_URL: &str = "https://s3.amazonaws.com/kru-downloads/da/DarkAges741single.exe";
const INSTALLER_FILE_NAME: &str = "DarkAges741single.exe";
const DEFAULT_EXE_OFFSET: u64 = 0x3A00;

pub(crate) enum InstallSource {
    Local(PathBuf),
    Download,
}

impl InstallSource {
    pub(crate) fn for_output(output: &Path) -> anyhow::Result<Self> {
        let output_dir = output.parent().context("installer output path must have a parent")?;
        let local_path = output_dir.join(INSTALLER_FILE_NAME);

        if local_path.exists() {
            Ok(Self::Local(local_path))
        } else {
            Ok(Self::Download)
        }
    }

    pub(crate) fn open(self) -> anyhow::Result<SourceReader> {
        match self {
            Self::Local(path) => {
                debug!(path = %path.display(), "Using local DarkAges741single.exe");
                let executable_offset = executable_offset_for_file(&path)?;
                let file = std::fs::File::open(&path)
                    .with_context(|| format!("failed to open {}", path.display()))?;

                Ok(SourceReader {
                    executable_offset,
                    stream: SourceStream::File(file),
                })
            }
            Self::Download => {
                debug!(url = INSTALLER_URL, "Streaming DarkAges741single.exe");

                let client = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(30))
                    .connect_timeout(std::time::Duration::from_secs(10))
                    .build()?;

                let response = client
                    .get(INSTALLER_URL)
                    .send()
                    .map_err(|error| anyhow::anyhow!("Download request failed: {error}"))?;

                if !response.status().is_success() {
                    return Err(anyhow::anyhow!(
                        "Download failed with status: {} (URL: {})",
                        response.status(),
                        INSTALLER_URL
                    ));
                }

                Ok(SourceReader {
                    executable_offset: DEFAULT_EXE_OFFSET,
                    stream: SourceStream::Http(response),
                })
            }
        }
    }
}

pub(crate) struct SourceReader {
    executable_offset: u64,
    stream: SourceStream,
}

impl SourceReader {
    pub(crate) fn executable_offset(&self) -> u64 {
        self.executable_offset
    }
}

impl Read for SourceReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.stream {
            SourceStream::File(file) => file.read(buf),
            SourceStream::Http(response) => response.read(buf),
        }
    }
}

enum SourceStream {
    File(std::fs::File),
    Http(reqwest::blocking::Response),
}

#[cfg(feature = "exe")]
fn executable_offset_for_file(path: &Path) -> anyhow::Result<u64> {
    let pe = exe::VecPE::from_disk_file(path)
        .with_context(|| format!("failed to inspect installer PE at {}", path.display()))?;
    let resource_section = pe
        .get_section_by_name(".rsrc")
        .context("installer PE is missing the .rsrc section")?;

    Ok(u64::from(
        resource_section.pointer_to_raw_data.0 + resource_section.size_of_raw_data,
    ))
}

#[cfg(not(feature = "exe"))]
fn executable_offset_for_file(_path: &Path) -> anyhow::Result<u64> {
    Ok(DEFAULT_EXE_OFFSET)
}