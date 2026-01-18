use bevy::prelude::Resource;
use rendering::texture_upload::{TextureUploadBelt, flush_pending_uploads};

#[derive(Resource, Default)]
pub struct GlobalUploadBelt(pub TextureUploadBelt);

impl GlobalUploadBelt {
    pub fn flush_all(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        player_mgr: Option<&mut rendering::scene::players::PlayerManager>,
        creature_mgr: Option<&mut rendering::scene::creatures::CreatureManager>,
    ) {
        if let Some(pm) = player_mgr {
            flush_pending_uploads(&mut pm.pending_uploads, device, encoder, &mut self.0);
        }
        if let Some(cm) = creature_mgr {
            flush_pending_uploads(&mut cm.pending_uploads, device, encoder, &mut self.0);
        }
    }
}