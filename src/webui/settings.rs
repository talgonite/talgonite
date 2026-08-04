pub use crate::settings::{
    CharacterPreview, SavedCredential, SavedCredentialPublic, ServerEntry, Settings as SettingsFile,
};

pub fn load_or_default() -> crate::settings::Settings {
    crate::settings::Settings::default()
}

pub fn save(_settings: &crate::settings::Settings) -> anyhow::Result<()> {
    Ok(())
}
