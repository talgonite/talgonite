//! Chat-related systems (speech bubbles, messages)

use super::super::components::*;
use crate::events::ChatEvent;
use bevy::prelude::*;
use packets::server::PublicMessageType;

const SPEECH_BUBBLE_DURATION_SECS: f32 = 3.0;
const CHANT_DURATION_SECS: f32 = 2.0;

/// Handles DisplayPublicMessage events and adds speech bubbles/chants to entities
pub fn handle_public_messages(
    mut commands: Commands,
    mut chat_events: MessageReader<ChatEvent>,
    entities_query: Query<(Entity, &EntityId)>,
) {
    for event in chat_events.read() {
        if let ChatEvent::PublicMessage(msg) = event {
            if let Some((entity, _)) = entities_query
                .iter()
                .find(|(_, eid)| eid.id == msg.source_id)
            {
                match msg.message_type {
                    PublicMessageType::Normal => {
                        commands.entity(entity).insert(SpeechBubble::new(
                            &msg.message,
                            SPEECH_BUBBLE_DURATION_SECS,
                            false,
                        ));
                    }
                    PublicMessageType::Shout => {
                        commands.entity(entity).insert(SpeechBubble::new(
                            &msg.message,
                            SPEECH_BUBBLE_DURATION_SECS,
                            true,
                        ));
                    }
                    PublicMessageType::Chant => {
                        commands
                            .entity(entity)
                            .insert(ChantLabel::new(&msg.message, CHANT_DURATION_SECS));
                    }
                }
            }
        }
    }
}

/// Removes expired speech bubbles
pub fn expire_speech_bubbles(
    mut commands: Commands,
    time: Res<Time>,
    mut bubbles_query: Query<(Entity, &mut SpeechBubble)>,
) {
    for (entity, mut bubble) in bubbles_query.iter_mut() {
        bubble.timer.tick(time.delta());
        if bubble.timer.is_finished() {
            commands.entity(entity).remove::<SpeechBubble>();
        }
    }
}

/// Removes expired chant labels
pub fn expire_chant_labels(
    mut commands: Commands,
    time: Res<Time>,
    mut chants_query: Query<(Entity, &mut ChantLabel)>,
) {
    for (entity, mut chant) in chants_query.iter_mut() {
        chant.timer.tick(time.delta());
        if chant.timer.is_finished() {
            commands.entity(entity).remove::<ChantLabel>();
        }
    }
}
