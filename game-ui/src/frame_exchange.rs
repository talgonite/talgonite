use bevy::prelude::*;
use std::sync::Mutex;

#[derive(Debug)]
pub enum ControlMessage {
    ReleaseFrontBufferTexture { texture: wgpu::Texture },
    ResizeBuffers { width: u32, height: u32, scale: f32 },
}

#[derive(Resource)]
pub struct FrameChannels {
    pub latest_front_buffer: Mutex<Option<wgpu::Texture>>,
    pub control_tx: smol::channel::Sender<ControlMessage>,
    pub control_rx: smol::channel::Receiver<ControlMessage>,
}

impl FrameChannels {
    pub fn new() -> Self {
        let (control_tx, control_rx) = smol::channel::bounded(8);
        Self {
            latest_front_buffer: Mutex::new(None),
            control_tx,
            control_rx,
        }
    }
}

#[derive(Resource, Default)]
pub struct BackBufferPool(pub Vec<wgpu::Texture>);
