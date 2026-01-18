use bevy::prelude::Resource;

/// Bevy resource wrapper around the renderer crate's `GlobalTextureUpload` so we don't need a
/// Bevy dependency inside the rendering crate.
#[derive(Resource)]
pub struct GlobalTextureUploadRes(pub rendering::GlobalTextureUpload);

impl GlobalTextureUploadRes {
    pub fn new(threshold: usize) -> Self { Self(rendering::GlobalTextureUpload::new(threshold)) }
}