use bevy::prelude::*;
use packets::client::{BeginChant, SpellChant, SpellUse, SpellUseArgs};
use packets::server::SpellType;

use crate::ecs::interaction::HoveredEntity;
use crate::events::{AbilityEvent, EntityClickEvent, TileClickEvent, WallClickEvent};
use crate::network::PacketOutbox;
use crate::webui::ipc::ActionId;
use crate::webui::plugin::AbilityState;

use super::components::{EntityId, NPC, Player, Position, TargetingHover};

#[derive(Resource, Default)]
pub struct SpellCastingState {
    pub active_cast: Option<ActiveSpellCast>,
}

#[derive(Resource, Default)]
pub struct SpellTargetingState {
    pub pending_target: Option<PendingTargetSpell>,
}

#[derive(Resource, Default)]
pub struct SpellQueueState {
    pub queued_spell: Option<QueuedSpellCast>,
}

pub struct ActiveSpellCast {
    pub spell_id: ActionId,
    pub spell_type: SpellType,
    pub total_cast_lines: u8,
    pub current_line: u8,
    pub time_since_last_chant: f32,
    pub target: Option<SpellTarget>,
}

pub struct PendingTargetSpell {
    pub spell_id: ActionId,
    pub total_cast_lines: u8,
}

pub struct QueuedSpellCast {
    pub spell_id: ActionId,
    pub slot: u8,
}

pub struct SpellTarget {
    pub entity_id: u32,
    pub position: (u16, u16),
}

pub fn start_spell_cast(
    mut events: MessageReader<AbilityEvent>,
    mut casting_state: ResMut<SpellCastingState>,
    mut targeting_state: ResMut<SpellTargetingState>,
    mut queue_state: ResMut<SpellQueueState>,
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

            if let Some(ref cast) = casting_state.active_cast {
                if remaining_cast_time(cast) <= 0.2 {
                    queue_state.queued_spell = Some(QueuedSpellCast {
                        spell_id: spell.id.clone(),
                        slot: *slot,
                    });
                    continue;
                }
            }

            if spell.spell_type == SpellType::NoTarget {
                targeting_state.pending_target = None;
                queue_state.queued_spell = None;
                casting_state.active_cast = None;

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
                        target: None,
                    });
                }
            } else {
                targeting_state.pending_target = Some(PendingTargetSpell {
                    spell_id: spell.id.clone(),
                    total_cast_lines: spell.cast_lines,
                });
            }
        }
    }
}

pub fn update_spell_casting(
    mut casting_state: ResMut<SpellCastingState>,
    mut queue_state: ResMut<SpellQueueState>,
    mut targeting_state: ResMut<SpellTargetingState>,
    ability_state: Option<Res<AbilityState>>,
    time: Res<Time>,
    outbox: Res<PacketOutbox>,
) {
    let Some(ref mut cast) = casting_state.active_cast else {
        return;
    };

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

            if cast.total_cast_lines >= 1 {
                let parsed_spell_name = crate::webui::ipc::parse_ability_name(&spell.panel_name);
                outbox.send(&SpellChant {
                    chant_message: parsed_spell_name.chant_name().to_string(),
                });
            }

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
            try_start_queued_spell(
                &mut casting_state,
                &mut targeting_state,
                &mut queue_state,
                Some(&**ability_state),
                &outbox,
            );
        }
    }
}

pub fn handle_spell_targeting(
    mut events: MessageReader<EntityClickEvent>,
    mut tile_clicks: MessageReader<TileClickEvent>,
    mut wall_clicks: MessageReader<WallClickEvent>,
    mut targeting_state: ResMut<SpellTargetingState>,
    mut queue_state: ResMut<SpellQueueState>,
    mut casting_state: ResMut<SpellCastingState>,
    ability_state: Option<Res<AbilityState>>,
    query: Query<(&EntityId, &Position, Option<&Player>, Option<&NPC>)>,
    outbox: Res<PacketOutbox>,
) {
    if tile_clicks.read().next().is_some() || wall_clicks.read().next().is_some() {
        targeting_state.pending_target = None;
        queue_state.queued_spell = None;
        return;
    }

    let Some(pending_target) = targeting_state.pending_target.as_ref() else {
        return;
    };

    for event in events.read() {
        if let Ok((entity_id, position, player, npc)) = query.get(event.entity) {
            if player.is_some() || npc.is_some() {
                let target = SpellTarget {
                    entity_id: entity_id.id,
                    position: (position.x as u16, position.y as u16),
                };

                let target_spell_id = pending_target.spell_id.clone();
                let target_cast_lines = pending_target.total_cast_lines;

                targeting_state.pending_target = None;
                queue_state.queued_spell = None;

                if target_cast_lines == 0 {
                    let Some(ref ability_state) = ability_state else {
                        casting_state.active_cast = None;
                        return;
                    };

                    let Some(spell) = ability_state
                        .spells
                        .iter()
                        .find(|s| s.id == target_spell_id)
                    else {
                        casting_state.active_cast = None;
                        return;
                    };

                    outbox.send(&SpellUse {
                        source_slot: spell.slot,
                        args: SpellUseArgs::Targeted {
                            target_id: target.entity_id,
                            target_x: target.position.0,
                            target_y: target.position.1,
                        },
                    });
                    casting_state.active_cast = None;
                } else {
                    outbox.send(&BeginChant {
                        cast_line_count: target_cast_lines,
                    });
                    outbox.send(&SpellChant {
                        chant_message: "1".to_string(),
                    });

                    casting_state.active_cast = Some(ActiveSpellCast {
                        spell_id: target_spell_id,
                        spell_type: SpellType::Targeted,
                        total_cast_lines: target_cast_lines,
                        current_line: 1,
                        time_since_last_chant: 0.0,
                        target: Some(target),
                    });
                }

                break;
            } else {
                break;
            }
        }
    }
}

pub fn update_targeting_hover(
    targeting_state: Res<SpellTargetingState>,
    active_drag: Res<crate::ecs::interaction::ActiveDragState>,
    hovered_entity: Res<HoveredEntity>,
    mut commands: Commands,
    targetable_query: Query<(
        Entity,
        Option<&Player>,
        Option<&NPC>,
        Option<&crate::ecs::components::LocalPlayer>,
    )>,
    with_hover: Query<Entity, With<TargetingHover>>,
) {
    let is_targeting = targeting_state.pending_target.is_some();
    let is_dragging_action = active_drag.is_dragging
        && (active_drag.source_panel == game_types::SlotPanelType::Item
            || active_drag.source_panel == game_types::SlotPanelType::Spell);

    if is_targeting || is_dragging_action {
        if let Some(hovered) = hovered_entity.0 {
            if let Ok((entity, player, npc, local_player)) = targetable_query.get(hovered) {
                let valid = if active_drag.is_dragging
                    && active_drag.source_panel == game_types::SlotPanelType::Item
                {
                    (player.is_some() && local_player.is_none()) || npc.is_some()
                } else {
                    player.is_some() || npc.is_some()
                };

                if valid && !with_hover.contains(entity) {
                    commands.entity(entity).insert(TargetingHover::default());
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

fn remaining_cast_time(cast: &ActiveSpellCast) -> f32 {
    if cast.total_cast_lines == 0 {
        return 0.0;
    }

    let chant_lines_remaining = cast.total_cast_lines.saturating_sub(cast.current_line) as f32;
    let time_until_next_line = (1.0 - cast.time_since_last_chant).clamp(0.0, 1.0);
    chant_lines_remaining + time_until_next_line
}

fn try_start_queued_spell(
    casting_state: &mut ResMut<SpellCastingState>,
    targeting_state: &mut ResMut<SpellTargetingState>,
    queue_state: &mut ResMut<SpellQueueState>,
    ability_state: Option<&AbilityState>,
    outbox: &Res<PacketOutbox>,
) {
    let Some(queued_spell) = queue_state.queued_spell.take() else {
        return;
    };

    let Some(ability_state) = ability_state else {
        return;
    };

    let Some(spell) = ability_state
        .spells
        .iter()
        .find(|s| s.id == queued_spell.spell_id)
    else {
        return;
    };

    match spell.spell_type {
        SpellType::Targeted => {
            targeting_state.pending_target = Some(PendingTargetSpell {
                spell_id: spell.id.clone(),
                total_cast_lines: spell.cast_lines,
            });
        }
        _ => {
            if spell.cast_lines == 0 {
                outbox.send(&SpellUse {
                    source_slot: queued_spell.slot,
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
                    target: None,
                });
            }
        }
    }
}
