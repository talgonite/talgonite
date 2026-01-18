use bevy::prelude::*;

#[derive(Debug)]
pub enum ControlMessage {
    ReleaseFrontBufferTexture { texture: wgpu::Texture },
    ResizeBuffers { width: u32, height: u32, scale: f32 },
}

#[derive(Resource)]
pub struct FrameChannels {
    pub front_buffer_tx: smol::channel::Sender<wgpu::Texture>,
    pub front_buffer_rx: smol::channel::Receiver<wgpu::Texture>,
    pub control_tx: smol::channel::Sender<ControlMessage>,
    pub control_rx: smol::channel::Receiver<ControlMessage>,
}

impl FrameChannels {
    pub fn new() -> Self {
        let (front_buffer_tx, front_buffer_rx) = smol::channel::bounded(3);
        let (control_tx, control_rx) = smol::channel::bounded(8);
        Self {
            front_buffer_tx,
            front_buffer_rx,
            control_tx,
            control_rx,
        }
    }
}

#[derive(Resource, Default)]
pub struct BackBufferPool(pub Vec<wgpu::Texture>);
