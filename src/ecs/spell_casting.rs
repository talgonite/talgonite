use bevy::prelude::*;
use packets::client::{BeginChant, SpellChant, SpellUse, SpellUseArgs};
use packets::server::SpellType;

use crate::ecs::interaction::HoveredEntity;
use crate::events::{AbilityEvent, EntityClickEvent};
use crate::network::PacketOutbox;
use crate::webui::ipc::ActionId;
use crate::webui::plugin::AbilityState;

use super::components::{EntityId, NPC, Player, Position, TargetingHover};

#[derive(Resource, Default)]
pub struct SpellCastingState {
    pub active_cast: Option<ActiveSpellCast>,
}

pub struct ActiveSpellCast {
    pub spell_id: ActionId,
    pub spell_type: SpellType,
    pub total_cast_lines: u8,
    pub current_line: u8,
    pub time_since_last_chant: f32,
    pub waiting_for_target: bool,
    pub target: Option<SpellTarget>,
}

pub struct SpellTarget {
    pub entity_id: u32,
    pub position: (u16, u16),
}

pub fn start_spell_cast(
    mut events: MessageReader<AbilityEvent>,
    mut casting_state: ResMut<SpellCastingState>,
    ability_state: Option<Res<AbilityState>>,
    outbox: Res<PacketOutbox>,
) {
    for event in events.read() {
        if let AbilityEvent::UseSpell { slot } = event {
            let Some(ref ability_state) = ability_state else {
                continue;
            };

            let Some(spell) = ability_state.spells.iter().find(|s| s.slot == *slot) else {
                continue;
            };

            casting_state.active_cast = None;

            match spell.spell_type {
                SpellType::Targeted => {
                    casting_state.active_cast = Some(ActiveSpellCast {
                        spell_id: spell.id.clone(),
                        spell_type: spell.spell_type,
                        total_cast_lines: spell.cast_lines,
                        current_line: 0,
                        time_since_last_chant: 0.0,
                        waiting_for_target: true,
                        target: None,
                    });
                }
                _ => {
                    if spell.cast_lines == 0 {
                        outbox.send(&SpellUse {
                            source_slot: *slot,
                            args: SpellUseArgs::None,
                        });
                    } else {
                        outbox.send(&BeginChant {
                            cast_line_count: spell.cast_lines,
                        });
                        outbox.send(&SpellChant {
                            chant_message: "1".to_string(),
                        });

                        casting_state.active_cast = Some(ActiveSpellCast {
                            spell_id: spell.id.clone(),
                            spell_type: spell.spell_type,
                            total_cast_lines: spell.cast_lines,
                            current_line: 1,
                            time_since_last_chant: 0.0,
                            waiting_for_target: false,
                            target: None,
                        });
                    }
                }
            }
        }
    }
}

pub fn update_spell_casting(
    mut casting_state: ResMut<SpellCastingState>,
    ability_state: Option<Res<AbilityState>>,
    time: Res<Time>,
    outbox: Res<PacketOutbox>,
) {
    let Some(ref mut cast) = casting_state.active_cast else {
        return;
    };

    if cast.waiting_for_target {
        return;
    }

    cast.time_since_last_chant += time.delta_secs();

    if cast.time_since_last_chant >= 1.0 {
        cast.current_line += 1;

        if cast.current_line <= cast.total_cast_lines {
            outbox.send(&SpellChant {
                chant_message: cast.current_line.to_string(),
            });
            cast.time_since_last_chant = 0.0;
        } else {
            let Some(ref ability_state) = ability_state else {
                casting_state.active_cast = None;
                return;
            };

            let Some(spell) = ability_state.spells.iter().find(|s| s.id == cast.spell_id) else {
                casting_state.active_cast = None;
                return;
            };

            let args = if let Some(ref target) = cast.target {
                SpellUseArgs::Targeted {
                    target_id: target.entity_id,
                    target_x: target.position.0,
                    target_y: target.position.1,
                }
            } else {
                SpellUseArgs::None
            };

            outbox.send(&SpellUse {
                source_slot: spell.slot,
                args,
            });

            casting_state.active_cast = None;
        }
    }
}

pub fn handle_spell_targeting(
    mut events: MessageReader<EntityClickEvent>,
    mut casting_state: ResMut<SpellCastingState>,
    ability_state: Option<Res<AbilityState>>,
    query: Query<(&EntityId, &Position, Option<&Player>, Option<&NPC>)>,
    outbox: Res<PacketOutbox>,
) {
    let Some(ref mut cast) = casting_state.active_cast else {
        return;
    };

    if !cast.waiting_for_target {
        return;
    }

    for event in events.read() {
        if let Ok((entity_id, position, player, npc)) = query.get(event.entity) {
            if player.is_some() || npc.is_some() {
                let target = SpellTarget {
                    entity_id: entity_id.id,
                    position: (position.x as u16, position.y as u16),
                };

                cast.target = Some(target);
                cast.waiting_for_target = false;

                if cast.total_cast_lines == 0 {
                    let Some(ref ability_state) = ability_state else {
                        casting_state.active_cast = None;
                        return;
                    };

                    let Some(spell) = ability_state.spells.iter().find(|s| s.id == cast.spell_id)
                    else {
                        casting_state.active_cast = None;
                        return;
                    };

                    let target_ref = cast.target.as_ref().unwrap();
                    outbox.send(&SpellUse {
                        source_slot: spell.slot,
                        args: SpellUseArgs::Targeted {
                            target_id: target_ref.entity_id,
                            target_x: target_ref.position.0,
                            target_y: target_ref.position.1,
                        },
                    });
                    casting_state.active_cast = None;
                } else {
                    outbox.send(&BeginChant {
                        cast_line_count: cast.total_cast_lines,
                    });
                    outbox.send(&SpellChant {
                        chant_message: "1".to_string(),
                    });

                    cast.current_line = 1;
                    cast.time_since_last_chant = 0.0;
                }

                break;
            } else {
                casting_state.active_cast = None;
                break;
            }
        }
    }
}

pub fn update_targeting_hover(
    casting_state: Res<SpellCastingState>,
    hovered_entity: Res<HoveredEntity>,
    mut commands: Commands,
    targetable_query: Query<(Entity, Option<&Player>, Option<&NPC>)>,
    with_hover: Query<Entity, With<TargetingHover>>,
) {
    let is_targeting = casting_state
        .active_cast
        .as_ref()
        .map_or(false, |c| c.waiting_for_target);

    if is_targeting {
        if let Some(hovered) = hovered_entity.0 {
            if let Ok((entity, player, npc)) = targetable_query.get(hovered) {
                if player.is_some() || npc.is_some() {
                    if !with_hover.contains(entity) {
                        commands.entity(entity).insert(TargetingHover::default());
                    }
                }
            }
        }

        for entity in with_hover.iter() {
            if Some(entity) != hovered_entity.0 {
                commands.entity(entity).remove::<TargetingHover>();
            }
        }
    } else {
        for entity in with_hover.iter() {
            commands.entity(entity).remove::<TargetingHover>();
        }
    }
}
