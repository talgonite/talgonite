use bevy::prelude::*;

use crate::app_state::AppState;
use bevy::tasks::Task;
use crc::crc16;
use std::any::type_name;
use std::time::Instant;

use crate::events::{
    AbilityEvent, AudioEvent, ChatEvent, EntityEvent, InventoryEvent, MapEvent, NetworkEvent,
    SessionEvent,
};
use packets::{TryFromBytes, client, server};

// Crossbeam receiver for network events coming from the background socket task
#[derive(Resource)]
pub struct NetEventRx(pub crossbeam_channel::Receiver<NetworkEvent>);

// Tracks per-session state used by the packet processor
#[derive(Resource)]
pub struct NetSessionState {
    start_time: Instant,
    map_download: MapDownloadState,
    metadata_requested: bool,
}

impl Default for NetSessionState {
    fn default() -> Self {
        Self {
            start_time: Instant::now(),
            map_download: MapDownloadState::None,
            metadata_requested: false,
        }
    }
}

#[derive(Debug)]
enum MapDownloadState {
    None,
    Requested {
        map_info: server::MapInfo,
        map_buf: Vec<u8>,
    },
}

impl Default for MapDownloadState {
    fn default() -> Self {
        MapDownloadState::None
    }
}

pub struct SessionRuntimePlugin;

impl Plugin for SessionRuntimePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NetSessionState>()
            .init_resource::<crate::network::PacketOutbox>()
            .init_resource::<crate::resources::PlayerAttributes>()
            .add_systems(
                PreUpdate,
                drain_net_events.run_if(in_state(AppState::InGame)),
            )
            .add_systems(Update, (process_net_packets, send_client_actions));
    }
}

// Drain the crossbeam receiver into Bevy events
fn drain_net_events(rx: Res<NetEventRx>, mut writer: MessageWriter<NetworkEvent>) {
    while let Ok(evt) = rx.0.try_recv() {
        writer.write(evt);
    }
}

// Marker component for the spawned TCP receive loop task
#[derive(Component)]
#[allow(dead_code)]
pub struct NetBgTask(pub Task<()>);

// Core network packet processor -> emits GameEvent and sends replies via PacketOutbox
fn process_net_packets(
    mut net_events: MessageReader<NetworkEvent>,
    mut session: ResMut<NetSessionState>,
    outbox: Res<crate::network::PacketOutbox>,
    mut player_attrs: ResMut<crate::resources::PlayerAttributes>,
    mut map_events: MessageWriter<MapEvent>,
    mut entity_events: MessageWriter<EntityEvent>,
    mut audio_events: MessageWriter<AudioEvent>,
    mut inventory_events: MessageWriter<InventoryEvent>,
    mut ability_events: MessageWriter<AbilityEvent>,
    mut chat_events: MessageWriter<ChatEvent>,
    mut session_events: MessageWriter<SessionEvent>,
    map_store: Res<crate::map_store::MapStore>,
    mut metafile_store: ResMut<crate::metafile_store::MetafileStore>,
    current_session: Option<Res<crate::CurrentSession>>,
) {
    let Some(current_session) = current_session else {
        return;
    };
    let server_id = current_session.server_id;
    metafile_store.set_server(server_id);

    for evt in net_events.read() {
        match evt {
            NetworkEvent::Connected => {
                session.start_time = Instant::now();
                tracing::info!("Network connected");
            }
            NetworkEvent::Disconnected => {
                tracing::warn!("Network disconnected");
            }
            NetworkEvent::Packet(code, data) => match code {
                &server::Codes::HeartBeatResponse => {
                    if let Some(query) = parse_packet::<server::HeartBeatResponse>(data) {
                        outbox.send(&client::HeartBeat { value: query.value });
                    }
                }
                &server::Codes::SynchronizeTicksResponse => {
                    if let Some(query) = parse_packet::<server::SynchronizeTicksResponse>(data) {
                        outbox.send(&client::SynchronizeTicks {
                            server_ticks: query.ticks as u32,
                            client_ticks: session.start_time.elapsed().as_millis() as u32,
                        });
                    }
                }
                &server::Codes::Sound => {
                    if let Some(q) = parse_packet::<server::Sound>(data) {
                        audio_events.write(AudioEvent::PlaySound(q));
                    }
                }
                &server::Codes::Location => {
                    if let Some(q) = parse_packet::<server::Location>(data) {
                        entity_events.write(EntityEvent::PlayerLocation(q));
                    }
                }
                &server::Codes::MapInfo => {
                    if let Some(q) = parse_packet::<server::MapInfo>(data) {
                        handle_map_info(
                            &mut session,
                            &outbox,
                            &mut map_events,
                            q,
                            &map_store,
                            server_id,
                        );
                    }
                }
                &server::Codes::MapData => {
                    if let Some(seg) = parse_packet::<server::MapData>(data) {
                        handle_map_data(&mut session, &mut map_events, seg, &map_store, server_id);
                    }
                }
                &server::Codes::ServerMessage => {
                    if let Some(q) = parse_packet::<server::ServerMessage>(data) {
                        if q.message != "" {
                            chat_events.write(ChatEvent::ServerMessage(q));
                        }
                    }
                }
                &server::Codes::DisplayPublicMessage => {
                    if let Some(q) = parse_packet::<server::DisplayPublicMessage>(data) {
                        chat_events.write(ChatEvent::PublicMessage(q));
                    }
                }
                &server::Codes::AddSkillToPane => {
                    if let Some(q) = parse_packet::<server::AddSkillToPane>(data) {
                        ability_events.write(AbilityEvent::AddSkill(q));
                    }
                }
                &server::Codes::WorldList => {
                    if let Some(q) = parse_packet::<server::WorldList>(data) {
                        session_events.write(SessionEvent::WorldList(q));
                    }
                }
                &server::Codes::RemoveSkillFromPane => {
                    if let Some(q) = parse_packet::<server::RemoveSkillFromPane>(data) {
                        ability_events.write(AbilityEvent::RemoveSkill(q));
                    }
                }
                &server::Codes::AddSpellToPane => {
                    if let Some(q) = parse_packet::<server::AddSpellToPane>(data) {
                        ability_events.write(AbilityEvent::AddSpell(q));
                    }
                }
                &server::Codes::RemoveSpellFromPane => {
                    if let Some(q) = parse_packet::<server::RemoveSpellFromPane>(data) {
                        ability_events.write(AbilityEvent::RemoveSpell(q));
                    }
                }
                &server::Codes::AddItemToPane => {
                    if let Some(q) = parse_packet::<server::AddItemToPane>(data) {
                        inventory_events.write(InventoryEvent::Add(q));
                    }
                }
                &server::Codes::HealthBar => {
                    if let Some(q) = parse_packet::<server::HealthBar>(data) {
                        entity_events.write(EntityEvent::HealthBar(q));
                    }
                }
                &server::Codes::UserId => {
                    if let Some(q) = parse_packet::<server::UserId>(data) {
                        session_events.write(SessionEvent::PlayerId(q.id));
                    }
                }
                &server::Codes::RemoveItemFromPane => {
                    if let Some(q) = parse_packet::<server::RemoveItemFromPane>(data) {
                        inventory_events.write(InventoryEvent::Remove(q));
                    }
                }
                &server::Codes::DisplayPlayer => {
                    if let Some(q) = parse_packet::<server::display_player::DisplayPlayer>(data) {
                        entity_events.write(EntityEvent::DisplayPlayer(q));
                    }
                }
                &server::Codes::DisplayVisibleEntities => {
                    if let Some(q) = parse_packet::<server::DisplayVisibleEntities>(data) {
                        entity_events.write(EntityEvent::DisplayEntities(q));
                    }
                }
                &server::Codes::CreatureWalk => {
                    if let Some(q) = parse_packet::<server::CreatureWalk>(data) {
                        entity_events.write(EntityEvent::Walk(q));
                    }
                }
                &server::Codes::ClientWalkResponse => {
                    if let Some(q) = parse_packet::<server::ClientWalkResponse>(data) {
                        entity_events.write(EntityEvent::PlayerWalkResponse(q));
                    }
                }
                &server::Codes::EntityTurn => {
                    if let Some(q) = parse_packet::<server::EntityTurn>(data) {
                        entity_events.write(EntityEvent::Turn(q));
                    }
                }
                &server::Codes::RemoveEntity => {
                    if let Some(q) = parse_packet::<server::RemoveEntity>(data) {
                        entity_events.write(EntityEvent::Remove(q));
                    }
                }
                &server::Codes::Attributes => {
                    if let Some(attrs) = parse_packet::<server::Attributes>(data) {
                        if let Some(vitality) = &attrs.vitality {
                            player_attrs.current_hp = vitality.current_hp;
                            player_attrs.current_mp = vitality.current_mp;
                        }
                        if let Some(primary) = &attrs.primary {
                            player_attrs.max_hp = primary.maximum_hp;
                            player_attrs.max_mp = primary.maximum_mp;
                        }
                    }
                }
                &server::Codes::Equipment => {
                    if let Some(q) = parse_packet::<server::Equipment>(data) {
                        inventory_events.write(InventoryEvent::Equipment(q));
                    }
                }
                &server::Codes::DisplayUnequip => {
                    if let Some(q) = parse_packet::<server::DisplayUnequip>(data) {
                        inventory_events.write(InventoryEvent::DisplayUnequip(q));
                    }
                }
                &server::Codes::EditableProfileRequest => {
                    if let Some(q) = parse_packet::<server::EditableProfileRequest>(data) {
                        tracing::info!("Received EditableProfileRequest: {:?}", q);
                    }
                }
                &server::Codes::MapChangePending => {
                    // Server indicates a map change will occur; clear current map entities now
                    map_events.write(MapEvent::Clear);
                    session.map_download = MapDownloadState::None;
                }
                &server::Codes::WorldMap => {
                    if let Some(q) = parse_packet::<server::WorldMap>(data) {
                        session_events.write(SessionEvent::WorldMap(q));
                    }
                }
                &server::Codes::SelfProfile => {
                    if let Some(q) = parse_packet::<server::SelfProfile>(data) {
                        session_events.write(SessionEvent::SelfProfile(q));
                    }
                }
                &server::Codes::OtherProfile => {
                    if let Some(q) = parse_packet::<server::OtherProfile>(data) {
                        session_events.write(SessionEvent::OtherProfile(q));
                    }
                }
                &server::Codes::DisplayMenu => {
                    if let Some(q) = parse_packet::<server::DisplayMenu>(data) {
                        tracing::info!("Received DisplayMenu: {:?}", q);
                        session_events.write(SessionEvent::DisplayMenu(q));
                    }
                }
                &server::Codes::LightLevel => {
                    if let Some(q) = parse_packet::<server::LightLevel>(data) {
                        map_events.write(MapEvent::SetLightLevel(q.kind));
                    }
                }
                &server::Codes::Door => {
                    if let Some(q) = parse_packet::<server::Door>(data) {
                        map_events.write(MapEvent::SetDoors(q));
                    }
                }
                &server::Codes::BodyAnimation => {
                    if let Some(q) = parse_packet::<server::BodyAnimation>(data) {
                        entity_events.write(EntityEvent::Animate(q));
                    }
                }
                &server::Codes::Animation => {
                    if let Some(q) = parse_packet::<server::Animation>(data) {
                        entity_events.write(EntityEvent::Effect(q));
                    }
                }
                &server::Codes::Cooldown => {
                    if let Some(q) = parse_packet::<server::Cooldown>(data) {
                        match q.kind {
                            packets::server::CooldownType::Skill => {
                                ability_events.write(AbilityEvent::SkillCooldown {
                                    slot: q.slot,
                                    cooldown_secs: q.cooldown_secs,
                                });
                            }
                            packets::server::CooldownType::Item => {
                                inventory_events.write(InventoryEvent::Cooldown {
                                    slot: q.slot,
                                    cooldown_secs: q.cooldown_secs,
                                });
                            }
                        }
                    }
                }
                &server::Codes::DisplayDialog => {
                    if let Some(q) = parse_packet::<server::DisplayDialog>(data) {
                        tracing::debug!("Received DisplayDialog: {:?}", q);
                        session_events.write(SessionEvent::DisplayDialog(q));
                    }
                }
                &server::Codes::MapLoadComplete => {
                    if let Some(_) = parse_packet::<server::MapLoadComplete>(data) {
                        handle_map_load_complete(&mut session, &outbox);
                    }
                }
                &server::Codes::MetaData => {
                    if let Some(q) = parse_packet::<server::MetaData>(data) {
                        handle_metadata(&outbox, &mut metafile_store, q);
                    }
                }
                e => {
                    tracing::warn!(?e, "Unhandled game event");
                }
            },
        }
    }
}

// Outbound: consume GameEvent actions and send corresponding client packets
fn send_client_actions(
    mut chat_events: MessageReader<ChatEvent>,
    mut inventory_events: MessageReader<InventoryEvent>,
    mut ability_events: MessageReader<AbilityEvent>,
    outbox: Res<crate::network::PacketOutbox>,
) {
    for e in chat_events.read() {
        match e {
            ChatEvent::SendPublicMessage(message, message_type) => {
                outbox.send(&client::PublicMessage {
                    public_message_type: *message_type,
                    message: message.clone(),
                });
            }
            ChatEvent::SendWhisper(target, message) => {
                outbox.send(&client::Whisper {
                    target_name: target.clone(),
                    message: message.clone(),
                });
            }
            ChatEvent::ServerMessage(_) | ChatEvent::PublicMessage(_) => {
                // Inbound only
            }
        }
    }
    for e in inventory_events.read() {
        match e {
            InventoryEvent::Swap { src, dst } => {
                outbox.send(&client::SwapSlot {
                    panel_type: client::SwapSlotPanelType::Inventory,
                    slot1: *src,
                    slot2: *dst,
                });
            }
            InventoryEvent::Use { slot } => {
                outbox.send(&client::ItemUse { source_slot: *slot });
            }
            InventoryEvent::Unequip { slot } => {
                use packets::types::EquipmentSlot;
                if let Ok(equipment_slot) = EquipmentSlot::try_from(*slot) {
                    outbox.send(&client::Unequip { equipment_slot });
                }
            }
            InventoryEvent::Add(_)
            | InventoryEvent::Remove(_)
            | InventoryEvent::Equipment(_)
            | InventoryEvent::DisplayUnequip(_)
            | InventoryEvent::Cooldown { .. } => {
                // Inbound only
            }
        }
    }
    for e in ability_events.read() {
        match e {
            AbilityEvent::UseSkill { slot } => {
                outbox.send(&client::SkillUse { source_slot: *slot });
            }

            AbilityEvent::UseSpell { .. }
            | AbilityEvent::SkillCooldown { .. }
            | AbilityEvent::AddSkill(_)
            | AbilityEvent::RemoveSkill(_)
            | AbilityEvent::AddSpell(_)
            | AbilityEvent::RemoveSpell(_) => {
                // Inbound only (UseSpell handled by spell_casting system)
            }
        }
    }
}

fn handle_map_info(
    session: &mut NetSessionState,
    outbox: &crate::network::PacketOutbox,
    map_events: &mut MessageWriter<MapEvent>,
    map_info: server::MapInfo,
    map_store: &crate::map_store::MapStore,
    server_id: u32,
) {
    let map_id = map_info.map_id;
    let checksum = map_info.check_sum;

    let cached_data = if let Some(data) = map_store.get_map(server_id, map_id) {
        let cached_checksum = crc16(&data);
        if cached_checksum == checksum {
            Some(data)
        } else {
            tracing::info!(
                map_id,
                "Cached map checksum mismatch (expected {}, got {})",
                checksum,
                cached_checksum
            );
            None
        }
    } else {
        None
    };

    if let Some(existing) = cached_data {
        tracing::info!(
            map_id = map_info.map_id,
            "Using cached map data for map id {}",
            map_info.map_id
        );
        map_events.write(MapEvent::SetInfo(
            map_info,
            std::sync::Arc::from(existing.into_boxed_slice()),
        ));
        session.map_download = MapDownloadState::None;
    } else {
        tracing::info!(
            map_id = map_info.map_id,
            "Requesting map data for map id {}",
            map_info.map_id
        );
        let total_size = map_info.height as usize * map_info.get_stride();
        if total_size == 0 {
            return;
        }
        session.map_download = MapDownloadState::Requested {
            map_info: map_info.clone(),
            map_buf: vec![0u8; total_size],
        };
        outbox.send(&client::MapDataRequest {
            x: map_info.width,
            y: map_info.height,
            checksum: [0u8; 3],
        });
    }
}

fn handle_map_data(
    session: &mut NetSessionState,
    map_events: &mut MessageWriter<MapEvent>,
    seg: server::MapData,
    map_store: &crate::map_store::MapStore,
    server_id: u32,
) {
    let MapDownloadState::Requested { map_info, map_buf } = &mut session.map_download else {
        return;
    };

    let stride = map_info.get_stride();
    if stride == 0 {
        session.map_download = MapDownloadState::None;
        return;
    }
    let start = seg.row as usize * stride;
    let end = start + stride;
    if end > map_buf.len() || seg.data.len() != stride {
        tracing::error!(
            map_id = map_info.map_id,
            row = seg.row,
            stride,
            data_len = seg.data.len(),
            buf_len = map_buf.len(),
            "Invalid map data segment received; cancelling download"
        );
        session.map_download = MapDownloadState::None;
        return;
    }
    map_buf[start..end].copy_from_slice(&seg.data);

    if end >= map_buf.len() {
        let checksum = crc16(&map_buf);
        if checksum == map_info.check_sum {
            map_store.save_map(server_id, map_info.map_id, &map_buf);
            let owned = std::mem::take(map_buf);
            map_events.write(MapEvent::SetInfo(
                map_info.clone(),
                std::sync::Arc::from(owned.into_boxed_slice()),
            ));
        } else {
            tracing::error!(
                map_id = map_info.map_id,
                expected = map_info.check_sum,
                actual = checksum,
                "Map download checksum mismatch"
            );
        }
        session.map_download = MapDownloadState::None;
    }
}

fn parse_packet<T: TryFromBytes>(data: &Vec<u8>) -> Option<T> {
    match T::try_from_bytes(data) {
        Ok(packet) => Some(packet),
        Err(err) => {
            tracing::error!(
                ?err,
                len = data.len(),
                packet = type_name::<T>(),
                "Failed to parse packet"
            );
            None
        }
    }
}

fn handle_map_load_complete(session: &mut NetSessionState, outbox: &crate::network::PacketOutbox) {
    // Only request metadata checksums once per session
    if !session.metadata_requested {
        tracing::info!("Map load complete, requesting metadata checksums");
        session.metadata_requested = true;
        outbox.send(&client::MetaDataRequest::AllCheckSums);
    }
}

fn handle_metadata(
    outbox: &crate::network::PacketOutbox,
    metafile_store: &mut crate::metafile_store::MetafileStore,
    metadata: server::MetaData,
) {
    match metadata {
        server::MetaData::AllCheckSums { collection } => {
            tracing::info!(
                "Received {} metadata checksums from server",
                collection.len()
            );

            // Request any metafiles that are missing or have mismatched checksums
            let mut request_count = 0;
            for entry in collection {
                let needs_download = match metafile_store.get_checksum(&entry.name) {
                    Some(local_checksum) => {
                        if local_checksum != entry.check_sum {
                            tracing::debug!(
                                "Metafile {} checksum mismatch (local {}, server {})",
                                entry.name,
                                local_checksum,
                                entry.check_sum
                            );
                            true
                        } else {
                            false
                        }
                    }
                    None => {
                        tracing::debug!("Metafile {} not found locally", entry.name);
                        true
                    }
                };

                if needs_download {
                    outbox.send(&client::MetaDataRequest::DataByName(entry.name));
                    request_count += 1;
                }
            }

            if request_count > 0 {
                tracing::info!("Requested {} metafiles from server", request_count);
            } else {
                tracing::info!("All metafiles are up to date");
            }
        }
        server::MetaData::DataByName {
            name,
            check_sum,
            data,
        } => {
            tracing::debug!(
                "Received metafile {} ({} bytes, checksum {})",
                name,
                data.len(),
                check_sum
            );

            // Save the metafile (this validates the checksum)
            metafile_store.save_metafile(&name, &data, check_sum);
        }
    }
}
