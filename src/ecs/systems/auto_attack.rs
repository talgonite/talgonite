use bevy::prelude::*;
use packets::client::Spacebar;

use crate::ecs::spell_casting::SpellCastingState;
use crate::events::PlayerAction;
use crate::network::PacketOutbox;

#[derive(Resource)]
pub struct AutoAttackState {
    enabled: bool,
    timer: Timer,
}

impl Default for AutoAttackState {
    fn default() -> Self {
        Self {
            enabled: false,
            // based on server-side assail intervals
            // retail/zol=1000ms, legends=900/450ms. all are multiples of 50ms
            timer: Timer::from_seconds(0.05, TimerMode::Once),
        }
    }
}

pub fn auto_attack_system(
    time: Res<Time>,
    mut actions: MessageReader<PlayerAction>,
    outbox: Res<PacketOutbox>,
    mut spell_casting: ResMut<SpellCastingState>,
    mut auto_attack: ResMut<AutoAttackState>,
) {
    let mut trigger_attack = false;

    for action in actions.read() {
        match action {
            PlayerAction::BasicAttack => {
                auto_attack.enabled = false;
                auto_attack.timer.reset();
                tracing::info!("Basic attack triggered");
                trigger_attack = true;
            }
            PlayerAction::ToggleAutoAttack => {
                auto_attack.enabled = !auto_attack.enabled;
                auto_attack.timer.reset();
                tracing::info!("Auto attack triggered");
                if auto_attack.enabled {
                    trigger_attack = true;
                }
            }
            _ => {}
        }
    }

    if auto_attack.enabled {
        auto_attack.timer.tick(time.delta());
        if auto_attack.timer.just_finished() {
            trigger_attack = true;
            auto_attack.timer.reset();
        }
    }

    if trigger_attack {
        spell_casting.active_cast = None;
        outbox.send(&Spacebar);
    }
}
