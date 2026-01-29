use bevy::prelude::*;
use packets::types::SocialStatus;

#[derive(Resource, Default)]
pub struct LocalSocialStatus {
    pub status: SocialStatus,
    pub pending_send: Option<SocialStatus>,
    pub version: u32,
}

impl LocalSocialStatus {
    pub fn set_status(&mut self, status: SocialStatus) {
        self.status = status;
        self.pending_send = Some(status);
        self.version = self.version.wrapping_add(1);
    }
    
    pub fn take_pending(&mut self) -> Option<SocialStatus> {
        self.pending_send.take()
    }
}
