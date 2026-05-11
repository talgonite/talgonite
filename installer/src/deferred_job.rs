use formats::epf::EpfImage;
use std::path::{Path, PathBuf};

pub(crate) enum DeferredAssetJob {
    Palette(PaletteAssetJob),
    Animation(AnimationAssetJob),
}

pub(crate) struct PaletteAssetJob {
    pub(crate) dat_name: String,
    pub(crate) dat_path: PathBuf,
    pub(crate) files_to_process: Vec<(String, Vec<u8>)>,
}

pub(crate) struct AnimationAssetJob {
    pub(crate) dat_name: String,
    pub(crate) epfs_to_concat: Vec<(String, EpfImage)>,
}

pub(crate) struct PaletteAssetJobBuilder {
    dat_name: String,
    dat_path: PathBuf,
    files_to_process: Vec<(String, Vec<u8>)>,
}

impl PaletteAssetJobBuilder {
    pub(crate) fn new(dat_name: &str, dat_path: &Path) -> Self {
        Self {
            dat_name: dat_name.to_string(),
            dat_path: dat_path.to_path_buf(),
            files_to_process: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, name: String, bytes: Vec<u8>) {
        self.files_to_process.push((name, bytes));
    }

    pub(crate) fn finish(self) -> Option<DeferredAssetJob> {
        if self.files_to_process.is_empty() {
            None
        } else {
            Some(DeferredAssetJob::Palette(PaletteAssetJob {
                dat_name: self.dat_name,
                dat_path: self.dat_path,
                files_to_process: self.files_to_process,
            }))
        }
    }
}

pub(crate) struct AnimationAssetJobBuilder {
    dat_name: String,
    epfs_to_concat: Vec<(String, EpfImage)>,
}

impl AnimationAssetJobBuilder {
    pub(crate) fn new(dat_name: &str) -> Self {
        Self {
            dat_name: dat_name.to_string(),
            epfs_to_concat: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, file_name: String, epf: EpfImage) {
        self.epfs_to_concat.push((file_name, epf));
    }

    pub(crate) fn finish(self) -> Option<DeferredAssetJob> {
        if self.epfs_to_concat.is_empty() {
            None
        } else {
            Some(DeferredAssetJob::Animation(AnimationAssetJob {
                dat_name: self.dat_name,
                epfs_to_concat: self.epfs_to_concat,
            }))
        }
    }
}