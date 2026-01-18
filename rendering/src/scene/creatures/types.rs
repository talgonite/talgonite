use etagere::Allocation;
use formats::mpf::{MpfAnimation, MpfAnimationType, MpfFile};

pub(crate) struct LoadedSprite {
    pub mpf_file: MpfFile,
    pub allocations: Vec<Allocation>,
    pub ref_count: usize,
}

pub struct AnimationData {
    pub frame_count: usize,
    pub start_frame_index: usize,
}

pub struct AddCreatureResult {
    pub handle: CreateInstanceHandle,
    pub animations: Vec<MpfAnimation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CreateInstanceHandle {
    pub index: usize,
    pub sprite_id: u16,
}

impl AddCreatureResult {
    pub fn get_animation(&self, anim_type: MpfAnimationType) -> Option<&MpfAnimation> {
        self.animations
            .iter()
            .find(|a| a.animation_type == anim_type)
    }
}
