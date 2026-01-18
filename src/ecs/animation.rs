use bevy::prelude::*;
use formats::{epf::EpfAnimationType, mpf::MpfAnimationType};

#[derive(Component)]
pub struct Animation {
    pub mode: AnimationMode,
    pub anim_type: AnimationType,
    pub current_frame: usize,
    pub end_index: usize,
    pub frame_duration: f32,
}

#[derive(Component)]
pub struct AnimationTimer(pub Timer);

#[derive(Bundle)]
pub struct AnimationBundle {
    pub animation: Animation,
    pub timer: AnimationTimer,
}

impl Animation {
    pub fn new(
        mode: AnimationMode,
        anim_type: AnimationType,
        frame_duration: f32,
        frame_count: usize,
    ) -> Animation {
        Animation {
            mode,
            anim_type,
            current_frame: 0,
            end_index: frame_count - 1,
            frame_duration,
        }
    }
}

impl AnimationBundle {
    pub fn new(
        mode: AnimationMode,
        anim_type: AnimationType,
        frame_duration: f32,
        frame_count: usize,
    ) -> Self {
        Self::from_animation(Animation::new(mode, anim_type, frame_duration, frame_count))
    }

    pub fn from_animation(animation: Animation) -> Self {
        let duration = animation.frame_duration;
        Self {
            animation,
            timer: AnimationTimer(Timer::from_seconds(duration, TimerMode::Repeating)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnimationType {
    Creature(MpfAnimationType),
    Player(EpfAnimationType),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnimationMode {
    OneShot,
    OneShotThenLoop {
        loop_anim: AnimationType,
        loop_frame_count: usize,
        loop_frame_duration: f32,
    },
    LoopStandard,
    LoopExtra {
        ratio: f32,
        standard_end: usize,
        extra_end: usize,
    },
    Finished,
}

pub fn animation_system(
    time: Res<Time>,
    mut query: Query<(&mut Animation, &mut AnimationTimer)>,
) {
    for (mut animation, mut timer) in query.iter_mut() {
        if animation.mode == AnimationMode::Finished {
            animation.bypass_change_detection();
            continue;
        }

        timer.0.tick(time.delta());

        if timer.0.just_finished() {
            if animation.current_frame < animation.end_index {
                animation.current_frame += 1;
            } else {
                match animation.mode {
                    AnimationMode::LoopStandard => {
                        animation.current_frame = 0;
                    }
                    AnimationMode::LoopExtra {
                        ratio,
                        standard_end,
                        extra_end,
                    } => {
                        animation.current_frame = 0;
                        let roll: f32 = rand::random();
                        animation.end_index = if roll < ratio {
                            standard_end
                        } else {
                            extra_end
                        }
                    }
                    AnimationMode::OneShot => {
                        animation.current_frame = 0;
                        animation.mode = AnimationMode::Finished;
                        // Keep component but stop ticking; will be detected as Finished (Idle)
                    }
                    AnimationMode::OneShotThenLoop {
                        loop_anim,
                        loop_frame_count,
                        loop_frame_duration,
                    } => {
                        animation.current_frame = 0;
                        animation.end_index = loop_frame_count - 1;
                        animation.anim_type = loop_anim;
                        timer.0 = Timer::from_seconds(loop_frame_duration, TimerMode::Repeating);

                        animation.mode = AnimationMode::LoopStandard;
                    }
                    AnimationMode::Finished => {
                        // Unreachable due to top-level skip
                    }
                }
            }
        } else {
            // Prevent triggering change detection unless the frame actually advanced
            animation.bypass_change_detection();
        }
    }
}
