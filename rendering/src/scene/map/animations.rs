use crate::{Instance, instance::InstanceFlag};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct WorldAnimation {
    pub ids: Vec<u16>,
    pub interval: Duration,
}

impl WorldAnimation {
    pub fn from_string(input: &str) -> Vec<WorldAnimation> {
        input
            .lines()
            .filter_map(|line| {
                let numbers: Vec<u16> = line
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();

                if numbers.len() <= 1 {
                    return None;
                }

                let interval = numbers.last().copied().unwrap_or(100);
                let ids = numbers[..numbers.len() - 1].to_vec();

                Some(WorldAnimation {
                    ids,
                    interval: Duration::from_millis((interval as u64) * 100),
                })
            })
            .collect()
    }
}

#[derive(Clone)]
pub struct InstanceReference {
    pub batch_index: usize,
    pub instance_index: usize,
}

#[derive(Clone)]
pub struct AnimationInstanceData {
    pub frame: usize,
    pub instances: Vec<InstanceReference>,
    pub frames: Vec<Instance>,
}

impl AnimationInstanceData {
    pub fn new(frames: Vec<Instance>) -> Self {
        Self {
            frame: 0,
            instances: Vec::new(),
            frames: convert_instance_positions_to_offset(&frames),
        }
    }

    pub fn advance(&mut self) -> Instance {
        self.frame = (self.frame + 1) % self.frames.len();
        self.frames[self.frame].clone()
    }

    pub fn set_frame(&mut self, target_frame: usize) -> Instance {
        let mut total_offset = glam::Vec3::ZERO;
        let target_frame = target_frame % self.frames.len();
        while self.frame != target_frame {
            self.frame = (self.frame + 1) % self.frames.len();
            total_offset += self.frames[self.frame].position;
        }
        let mut result = self.frames[self.frame].clone();
        result.position = total_offset;
        result
    }
}

#[derive(Clone)]
pub struct WorldAnimationInstanceData {
    pub data: AnimationInstanceData,
    next_update: Instant,
    animation: WorldAnimation,
}

#[derive(Clone)]
pub struct ManualAnimationInstanceData {
    pub data: AnimationInstanceData,
}

impl WorldAnimationInstanceData {
    pub fn new(animation: WorldAnimation, frames: Vec<Instance>) -> Self {
        let now = Instant::now();

        WorldAnimationInstanceData {
            data: AnimationInstanceData::new(frames),
            next_update: now + animation.interval,
            animation,
        }
    }

    pub fn advance(&mut self, now: Instant) -> Instance {
        self.next_update = now + self.animation.interval;
        self.data.advance()
    }

    pub fn should_update(&self, now: Instant) -> bool {
        now >= self.next_update
    }

    pub fn contains_id(&self, id: u16) -> bool {
        self.animation.ids.contains(&id)
    }

    pub fn instances(&self) -> &[InstanceReference] {
        &self.data.instances
    }

    pub fn instances_mut(&mut self) -> &mut Vec<InstanceReference> {
        &mut self.data.instances
    }
}

fn convert_instance_positions_to_offset(frames: &[Instance]) -> Vec<Instance> {
    frames
        .iter()
        .enumerate()
        .map(|(i, frame)| {
            let previous_frame = if i == 0 {
                frames.last().unwrap()
            } else {
                &frames[i - 1]
            };
            Instance {
                position: frame.position - previous_frame.position,
                tex_min: frame.tex_min,
                tex_max: frame.tex_max,
                sprite_size: frame.sprite_size,
                palette_offset: frame.palette_offset,
                dye_v_offset: -1.,
                flags: InstanceFlag::None,
                tint: glam::Vec3::ZERO,
            }
        })
        .collect()
}
