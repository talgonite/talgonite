//! Profile panel bridge - syncs player profile state to Slint UI.

use bevy::prelude::*;
use packets::server::EquipmentSlot;

use crate::slint_support::state_bridge::{SlintAssetLoaderRes, SlintWindow};
use crate::{EquipmentSlotData, GameState, LegendMarkData, ProfileData, RendererState};

/// Event emitted when the player wants to show a profile panel
#[derive(Debug, Clone, Message)]
pub enum ShowSelfProfileEvent {
    SelfRequested,  // User double-clicked self
    SelfUpdate,     // Server sent SelfProfile packet
    OtherRequested, // User double-clicked other (optimistic UI)
    OtherUpdate,    // Server sent OtherProfile packet
}

/// Maps legend mark color string to Slint color.
pub fn legend_mark_color(color_str: &str) -> slint::Color {
    match color_str {
        c if c.contains("Red") => slint::Color::from_rgb_u8(255, 100, 100),
        c if c.contains("Blue") => slint::Color::from_rgb_u8(100, 100, 255),
        c if c.contains("Green") => slint::Color::from_rgb_u8(100, 255, 100),
        c if c.contains("Yellow") => slint::Color::from_rgb_u8(255, 255, 100),
        c if c.contains("Orange") => slint::Color::from_rgb_u8(255, 165, 0),
        c if c.contains("Purple") => slint::Color::from_rgb_u8(160, 32, 240),
        c if c.contains("Cyan") => slint::Color::from_rgb_u8(0, 255, 255),
        c if c.contains("White") => slint::Color::from_rgb_u8(255, 255, 255),
        _ => slint::Color::from_rgb_u8(200, 200, 200),
    }
}

/// Build equipment slot data from item info.
pub fn build_equipment_slot(
    asset_loader: &crate::slint_support::assets::SlintAssetLoader,
    gf: &crate::game_files::GameFiles,
    sprite: u16,
    name: Option<&str>,
    current_durability: u32,
    max_durability: u32,
) -> EquipmentSlotData {
    let durability_percent = if max_durability > 0 {
        current_durability as f32 / max_durability as f32
    } else {
        1.0
    };
    EquipmentSlotData {
        name: slint::SharedString::from(name.unwrap_or_default()),
        icon: asset_loader.load_item_icon(gf, sprite).unwrap_or_default(),
        has_item: true,
        durability_percent,
        current_durability: current_durability as i32,
        max_durability: max_durability as i32,
    }
}

/// System that syncs PlayerProfileState to Slint whenever it changes
pub fn sync_profile_to_slint(
    win: Res<SlintWindow>,
    asset_loader: Res<SlintAssetLoaderRes>,
    game_files: Res<crate::game_files::GameFiles>,
    eq_state: Res<crate::webui::plugin::EquipmentState>,
    profile_state: Res<crate::webui::plugin::PlayerProfileState>,
    mut portrait_state: ResMut<crate::resources::ProfilePortraitState>,
    renderer: Res<RendererState>,
    mut last_portrait_version: Local<u32>,
) {
    let Some(strong) = win.0.upgrade() else {
        return;
    };
    let asset_loader = &asset_loader.0;

    let mut portrait_image = None;
    if portrait_state.version != *last_portrait_version {
        let profile_size = 128;
        let next_texture = rendering::texture::Texture::create_render_texture(
            &renderer.device,
            "profile_portrait",
            profile_size,
            profile_size,
            wgpu::TextureFormat::Rgba8Unorm,
        );

        let old_texture = std::mem::replace(&mut portrait_state.texture, next_texture.texture);
        portrait_state.view = next_texture.view;

        if let Ok(image) = old_texture.try_into() {
            portrait_image = Some(image);
        }
        *last_portrait_version = portrait_state.version;
    }

    if profile_state.is_changed() || portrait_image.is_some() {
        let game_state = slint::ComponentHandle::global::<GameState>(&strong);
        let mut profile = game_state.get_profile();

        if let Some(img) = portrait_image {
            profile.preview = img;
        }

        if !profile_state.name.is_empty() {
            profile.name = slint::SharedString::from(profile_state.name.as_str());
        }
        profile.class = slint::SharedString::from(profile_state.class.as_str());
        profile.guild = slint::SharedString::from(profile_state.guild.as_str());
        profile.guild_rank = slint::SharedString::from(profile_state.guild_rank.as_str());
        profile.title = slint::SharedString::from(profile_state.title.as_str());
        profile.town = slint::SharedString::from(format!("{:?}", profile_state.nation));
        profile.group_requests_enabled = profile_state.group_open;
        profile.profile_text = slint::SharedString::from(profile_state.profile_text.to_plain_string());

        let legend_marks: Vec<LegendMarkData> = profile_state
            .legend_marks
            .iter()
            .map(|m| LegendMarkData {
                icon_name: slint::SharedString::from(format!("{:?}", m.icon)),
                color: legend_mark_color(&format!("{:?}", m.color)),
                text: slint::SharedString::from(m.text.as_str()),
            })
            .collect();
        profile.legend_marks = slint::ModelRc::new(slint::VecModel::from(legend_marks));

        // Sync equipment as well if changed
        let is_other_player = !profile_state.name.is_empty();

        let make_slot = |slot_type: EquipmentSlot| {
            if is_other_player {
                if let Some(item) = profile_state.equipment.get(&slot_type) {
                    return build_equipment_slot(asset_loader, &game_files, item.sprite, None, 0, 0);
                }
            } else if let Some(item) = eq_state.0.get(&slot_type) {
                return build_equipment_slot(
                    asset_loader,
                    &game_files,
                    item.sprite,
                    Some(&item.name),
                    item.current_durability,
                    item.max_durability,
                );
            }
            EquipmentSlotData::default()
        };

        profile.eq_weapon = make_slot(EquipmentSlot::Weapon);
        profile.eq_armor = make_slot(EquipmentSlot::Armor);
        profile.eq_shield = make_slot(EquipmentSlot::Shield);
        profile.eq_helmet = make_slot(EquipmentSlot::Helmet);
        profile.eq_earrings = make_slot(EquipmentSlot::Earrings);
        profile.eq_necklace = make_slot(EquipmentSlot::Necklace);
        profile.eq_left_ring = make_slot(EquipmentSlot::LeftRing);
        profile.eq_right_ring = make_slot(EquipmentSlot::RightRing);
        profile.eq_left_gauntlet = make_slot(EquipmentSlot::LeftGaunt);
        profile.eq_right_gauntlet = make_slot(EquipmentSlot::RightGaunt);
        profile.eq_belt = make_slot(EquipmentSlot::Belt);
        profile.eq_greaves = make_slot(EquipmentSlot::Greaves);
        profile.eq_boots = make_slot(EquipmentSlot::Boots);
        profile.eq_accessory1 = make_slot(EquipmentSlot::Accessory1);
        profile.eq_accessory2 = make_slot(EquipmentSlot::Accessory2);
        profile.eq_overcoat = make_slot(EquipmentSlot::Overcoat);
        profile.eq_over_helmet = make_slot(EquipmentSlot::OverHelm);
        profile.eq_over_armor = make_slot(EquipmentSlot::Accessory3);

        game_state.set_profile(profile);
    }
}

/// System that handles ShowSelfProfileEvent to display the profile panel
pub fn handle_show_self_profile(
    mut reader: MessageReader<ShowSelfProfileEvent>,
    win: Res<SlintWindow>,
    asset_loader: Res<SlintAssetLoaderRes>,
    game_files: Res<crate::game_files::GameFiles>,
    eq_state: Res<crate::webui::plugin::EquipmentState>,
    mut profile_state: ResMut<crate::webui::plugin::PlayerProfileState>,
    mut portrait_state: ResMut<crate::resources::ProfilePortraitState>,
) {
    let Some(strong) = win.0.upgrade() else {
        return;
    };
    let asset_loader = &asset_loader.0;

    for event in reader.read() {
        let game_state = slint::ComponentHandle::global::<GameState>(&strong);

        match event {
            ShowSelfProfileEvent::OtherRequested => {
                // When requesting another player, clear stale state and HIDE the panel
                // until we get the actual data from the server.
                profile_state.clear();
                let mut profile = game_state.get_profile();
                profile.visible = false;
                game_state.set_profile(profile);

                portrait_state.dirty = true;
                continue;
            }
            ShowSelfProfileEvent::SelfRequested => {
                // When requesting our own profile, clear the "other player" state so we use
                // our own local EquipmentState and Name optimistically.
                profile_state.clear();

                portrait_state.dirty = true;
            }
            ShowSelfProfileEvent::SelfUpdate => {
                // If this is a response from the server but the user closed the panel already, don't reopen it
                let current = game_state.get_profile();
                if !current.visible {
                    continue;
                }

                portrait_state.dirty = true;
            }
            ShowSelfProfileEvent::OtherUpdate => {
                // Server sent detail for another player - update and ensure it's handled below
                portrait_state.dirty = true;
            }
        }

        // Get current player name to use in profile
        let player_name = game_state.get_player_name();

        let mut profile = ProfileData {
            visible: true,
            is_self: true,
            name: player_name,
            preview: portrait_state
                .texture
                .clone()
                .try_into()
                .unwrap_or_default(),
            ..Default::default()
        };

        // Populate profile fields from state
        if !profile_state.name.is_empty() {
            profile.name = slint::SharedString::from(profile_state.name.as_str());
        }
        profile.is_self = profile_state.is_self;
        profile.class = slint::SharedString::from(profile_state.class.as_str());
        profile.guild = slint::SharedString::from(profile_state.guild.as_str());
        profile.guild_rank = slint::SharedString::from(profile_state.guild_rank.as_str());
        profile.title = slint::SharedString::from(profile_state.title.as_str());
        profile.town = slint::SharedString::from(format!("{:?}", profile_state.nation));
        profile.group_requests_enabled = profile_state.group_open;
        profile.profile_text = slint::SharedString::from(profile_state.profile_text.to_plain_string());

        let legend_marks: Vec<LegendMarkData> = profile_state
            .legend_marks
            .iter()
            .map(|m| LegendMarkData {
                icon_name: slint::SharedString::from(format!("{:?}", m.icon)),
                color: legend_mark_color(&format!("{:?}", m.color)),
                text: slint::SharedString::from(m.text.as_str()),
            })
            .collect();
        profile.legend_marks = slint::ModelRc::new(slint::VecModel::from(legend_marks));

        // Populate equipment if available
        let is_other_player = !profile_state.name.is_empty();

        let make_slot = |slot_type: EquipmentSlot| {
            // Try to get from profile_state first (set for other players' profiles)
            if is_other_player {
                if let Some(item) = profile_state.equipment.get(&slot_type) {
                    return build_equipment_slot(asset_loader, &game_files, item.sprite, None, 0, 0);
                }
                return EquipmentSlotData::default();
            }

            // Fall back to local player's equipment state (only for self profile)
            if let Some(item) = eq_state.0.get(&slot_type) {
                return build_equipment_slot(
                    asset_loader,
                    &game_files,
                    item.sprite,
                    Some(&item.name),
                    item.current_durability,
                    item.max_durability,
                );
            }

            EquipmentSlotData::default()
        };

        profile.eq_weapon = make_slot(EquipmentSlot::Weapon);
        profile.eq_armor = make_slot(EquipmentSlot::Armor);
        profile.eq_shield = make_slot(EquipmentSlot::Shield);
        profile.eq_helmet = make_slot(EquipmentSlot::Helmet);
        profile.eq_earrings = make_slot(EquipmentSlot::Earrings);
        profile.eq_necklace = make_slot(EquipmentSlot::Necklace);
        profile.eq_left_ring = make_slot(EquipmentSlot::LeftRing);
        profile.eq_right_ring = make_slot(EquipmentSlot::RightRing);
        profile.eq_left_gauntlet = make_slot(EquipmentSlot::LeftGaunt);
        profile.eq_right_gauntlet = make_slot(EquipmentSlot::RightGaunt);
        profile.eq_belt = make_slot(EquipmentSlot::Belt);
        profile.eq_greaves = make_slot(EquipmentSlot::Greaves);
        profile.eq_boots = make_slot(EquipmentSlot::Boots);
        profile.eq_accessory1 = make_slot(EquipmentSlot::Accessory1);
        profile.eq_accessory2 = make_slot(EquipmentSlot::Accessory2);
        profile.eq_overcoat = make_slot(EquipmentSlot::Overcoat);
        profile.eq_over_helmet = make_slot(EquipmentSlot::OverHelm);
        // Accessory3 -> eq_over_armor (best guess for now)
        profile.eq_over_armor = make_slot(EquipmentSlot::Accessory3);

        game_state.set_profile(profile);
        tracing::info!("Showing self profile panel");
    }
}
