use bevy::prelude::*;
use formats::{epf::EpfAnimationType, mpf::MpfAnimationType};


#[derive(Component, Clone, Debug, PartialEq)]
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
        let frame_count = frame_count.max(1);
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

#[derive(Clone, Debug, PartialEq)]
pub enum AnimationMode {
    OneShot,
    OneShotThen(Box<Animation>),
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
                    }
                    AnimationMode::OneShotThen(ref next) => {
                        let next_anim = (**next).clone();
                        timer.0 = Timer::from_seconds(next_anim.frame_duration, TimerMode::Repeating);
                        *animation = next_anim;
                    }
                    AnimationMode::Finished => {}
                }
            }
        } else {
            animation.bypass_change_detection();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    #[test]
    fn one_shot_then_transitions_to_next_animation() {
        let mut app = App::new();
        app.add_plugins(bevy::time::TimePlugin::default());
        app.add_systems(Update, animation_system);

        let idle_anim = Animation::new(
            AnimationMode::LoopStandard,
            AnimationType::Creature(MpfAnimationType::Standing),
            0.5,
            2,
        );
        let walk_bundle = AnimationBundle::new(
            AnimationMode::OneShotThen(Box::new(idle_anim.clone())),
            AnimationType::Creature(MpfAnimationType::Walk),
            0.25,
            2,
        );

        let entity = app.world_mut().spawn(walk_bundle).id();
        app.update();

        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(250)));
        app.update();

        let anim = app.world().get::<Animation>(entity).unwrap();
        assert_eq!(anim.current_frame, 1);
        assert_eq!(anim.anim_type, AnimationType::Creature(MpfAnimationType::Walk));

        app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(250)));
        app.update();

        let anim = app.world().get::<Animation>(entity).unwrap();
        assert_eq!(anim.current_frame, 0);
        assert_eq!(anim.anim_type, AnimationType::Creature(MpfAnimationType::Standing));
        assert_eq!(anim.mode, AnimationMode::LoopStandard);
    }
}
