use bevy::prelude::*;

use crate::game_files::GameFiles;
use crate::slint_support::assets::SlintAssetLoader;
use crate::slint_support::state_bridge::SlintAssetLoaderRes;
use crate::events::MapEvent;
use crate::ecs::components::InGameScoped;
use crate::resources::PlayerAttributes;
use crate::webui::plugin::{
    AbilityState, ActiveMenuContext, EquipmentState, InventoryState, PlayerProfileState,
    WorldListState,
};
use crate::{MapRendererState, network::{NetworkManager, PacketOutbox}};
use crate::ecs::hotbar::{HotbarPanelState, HotbarState};
use crate::session::runtime::{NetBgTask, NetEventRx, NetSessionState};

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Hash, States)]
pub enum AppState {
    #[default]
    Installing,
    MainMenu,
    InGame,
}

pub fn setup_game_files(mut commands: Commands, existing: Option<Res<GameFiles>>) {
    if existing.is_some() {
        return;
    }

    let game_files = GameFiles::new();
    commands.insert_resource(SlintAssetLoaderRes(SlintAssetLoader::new(&game_files)));
    commands.insert_resource(game_files);
}

pub fn cleanup_game_files(mut commands: Commands) {
    commands.remove_resource::<SlintAssetLoaderRes>();
    commands.remove_resource::<GameFiles>();
}

pub fn cleanup_ingame_world(
    mut commands: Commands,
    scoped_q: Query<Entity, With<InGameScoped>>,
    mut map_events: MessageWriter<MapEvent>,
) {
    for e in scoped_q.iter() {
        commands.entity(e).despawn();
    }
    commands.remove_resource::<MapRendererState>();
    commands.remove_resource::<crate::ecs::collision::MapCollisionData>();
    map_events.write(MapEvent::Clear);
}

pub fn cleanup_ingame_resources(
    mut commands: Commands,
    net_tasks: Query<Entity, With<NetBgTask>>,
    inventory: Option<ResMut<InventoryState>>,
    ability: Option<ResMut<AbilityState>>,
    world_list: Option<ResMut<WorldListState>>,
    equipment: Option<ResMut<EquipmentState>>,
    profile: Option<ResMut<PlayerProfileState>>,
    hotbar: Option<ResMut<HotbarState>>,
    hotbar_panel: Option<ResMut<HotbarPanelState>>,
    player_attrs: Option<ResMut<PlayerAttributes>>,
    menu_ctx: Option<ResMut<ActiveMenuContext>>,
    session: Option<ResMut<NetSessionState>>,
    outbox: Option<ResMut<PacketOutbox>>,
) {
    for e in net_tasks.iter() {
        commands.entity(e).despawn();
    }

    commands.remove_resource::<NetworkManager>();
    commands.remove_resource::<NetEventRx>();

    if let Some(mut state) = inventory {
        *state = InventoryState::default();
    }
    if let Some(mut state) = ability {
        *state = AbilityState::default();
    }
    if let Some(mut state) = world_list {
        *state = WorldListState::default();
    }
    if let Some(mut state) = equipment {
        *state = EquipmentState::default();
    }
    if let Some(mut state) = profile {
        *state = PlayerProfileState::default();
    }
    if let Some(mut state) = hotbar {
        *state = HotbarState::default();
    }
    if let Some(mut state) = hotbar_panel {
        *state = HotbarPanelState::default();
    }
    if let Some(mut state) = player_attrs {
        *state = PlayerAttributes::default();
    }
    if let Some(mut state) = menu_ctx {
        state.entity_type = None;
        state.entity_id = 0;
        state.pursuit_id = 0;
        state.menu_type = None;
        state.args.clear();
    }
    if let Some(mut state) = session {
        *state = NetSessionState::default();
    }
    if let Some(mut state) = outbox {
        *state = PacketOutbox::default();
    }
}
