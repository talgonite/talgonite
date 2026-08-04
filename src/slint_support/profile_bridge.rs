//! Profile panel bridge - syncs player profile state to Slint UI.

use bevy::prelude::*;
use packets::server::EquipmentSlot;

use crate::slint_support::assets::{IconKind, SlintAssetLoader};
use crate::slint_support::state_bridge::{u8_to_social_status, SlintAssetLoaderRes, SlintWindow};
use crate::{EquipmentSlotData, GameState, LegendMarkData, ProfileData};

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

/// Raw sprite + label info needed to render one equipment slot, kept separate
/// from the icon so a whole panel can be batched through the icon loader.
struct ProfileSlotSpec {
    sprite: u16,
    name: Option<String>,
    current_durability: u32,
    max_durability: u32,
}

const PROFILE_EQUIPMENT_ORDER: [EquipmentSlot; 18] = [
    EquipmentSlot::Weapon,
    EquipmentSlot::Armor,
    EquipmentSlot::Shield,
    EquipmentSlot::Helmet,
    EquipmentSlot::Earrings,
    EquipmentSlot::Necklace,
    EquipmentSlot::LeftRing,
    EquipmentSlot::RightRing,
    EquipmentSlot::LeftGaunt,
    EquipmentSlot::RightGaunt,
    EquipmentSlot::Belt,
    EquipmentSlot::Greaves,
    EquipmentSlot::Boots,
    EquipmentSlot::Accessory1,
    EquipmentSlot::Accessory2,
    EquipmentSlot::Overcoat,
    EquipmentSlot::OverHelm,
    EquipmentSlot::Accessory3,
];

fn collect_profile_slot_specs(
    is_other_player: bool,
    profile_equipment: &std::collections::HashMap<EquipmentSlot, packets::types::ItemInfo>,
    equipment: &std::collections::HashMap<EquipmentSlot, packets::server::Equipment>,
) -> [Option<ProfileSlotSpec>; 18] {
    std::array::from_fn(|index| {
        let slot = PROFILE_EQUIPMENT_ORDER[index];
        if is_other_player {
            profile_equipment.get(&slot).map(|item| ProfileSlotSpec {
                sprite: item.sprite,
                name: None,
                current_durability: 0,
                max_durability: 0,
            })
        } else {
            equipment.get(&slot).map(|item| ProfileSlotSpec {
                sprite: item.sprite,
                name: Some(item.name.clone()),
                current_durability: item.current_durability,
                max_durability: item.max_durability,
            })
        }
    })
}

/// Load all 18 equipment icons in one batched, parallel pass and build the
/// Slint slot data.
fn build_equipment_slots(
    asset_loader: &SlintAssetLoader,
    game_files: &crate::game_files::GameFiles,
    specs: &[Option<ProfileSlotSpec>; 18],
) -> [EquipmentSlotData; 18] {
    let requests: Vec<(IconKind, u16)> = specs
        .iter()
        .flatten()
        .map(|spec| (IconKind::Item, spec.sprite))
        .collect();
    let mut icons = asset_loader.icons(game_files, &requests).into_iter();

    std::array::from_fn(|index| {
        let Some(spec) = specs[index].as_ref() else {
            return EquipmentSlotData::default();
        };
        let durability_percent = if spec.max_durability > 0 {
            spec.current_durability as f32 / spec.max_durability as f32
        } else {
            1.0
        };
        EquipmentSlotData {
            name: slint::SharedString::from(spec.name.as_deref().unwrap_or_default()),
            icon: icons.next().flatten().unwrap_or_default(),
            has_item: true,
            durability_percent,
            current_durability: spec.current_durability as i32,
            max_durability: spec.max_durability as i32,
        }
    })
}

/// Copy the 18 slots onto the profile fields, in panel display order.
fn assign_equipment(profile: &mut ProfileData, slots: [EquipmentSlotData; 18]) {
    let [
        eq_weapon,
        eq_armor,
        eq_shield,
        eq_helmet,
        eq_earrings,
        eq_necklace,
        eq_left_ring,
        eq_right_ring,
        eq_left_gauntlet,
        eq_right_gauntlet,
        eq_belt,
        eq_greaves,
        eq_boots,
        eq_accessory1,
        eq_accessory2,
        eq_overcoat,
        eq_over_helmet,
        eq_over_armor,
    ] = slots;

    profile.eq_weapon = eq_weapon;
    profile.eq_armor = eq_armor;
    profile.eq_shield = eq_shield;
    profile.eq_helmet = eq_helmet;
    profile.eq_earrings = eq_earrings;
    profile.eq_necklace = eq_necklace;
    profile.eq_left_ring = eq_left_ring;
    profile.eq_right_ring = eq_right_ring;
    profile.eq_left_gauntlet = eq_left_gauntlet;
    profile.eq_right_gauntlet = eq_right_gauntlet;
    profile.eq_belt = eq_belt;
    profile.eq_greaves = eq_greaves;
    profile.eq_boots = eq_boots;
    profile.eq_accessory1 = eq_accessory1;
    profile.eq_accessory2 = eq_accessory2;
    profile.eq_overcoat = eq_overcoat;
    profile.eq_over_helmet = eq_over_helmet;
    // Accessory3 -> eq_over_armor (best guess for now)
    profile.eq_over_armor = eq_over_armor;
}

/// System that syncs PlayerProfileState to Slint whenever it changes
pub fn sync_profile_to_slint(
    win: Res<SlintWindow>,
    asset_loader: Res<SlintAssetLoaderRes>,
    game_files: Res<crate::game_files::GameFiles>,
    eq_state: Res<crate::webui::plugin::EquipmentState>,
    profile_state: Res<crate::webui::plugin::PlayerProfileState>,
    portrait_state: Res<crate::resources::ProfilePortraitState>,
    mut last_portrait_version: Local<u32>,
) {
    let Some(strong) = win.0.upgrade() else {
        return;
    };
    let asset_loader = &asset_loader.0;

    let mut portrait_image = None;
    if portrait_state.version != *last_portrait_version {
        if let Ok(image) = portrait_state.texture.clone().try_into() {
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
        profile.social_status = u8_to_social_status(profile_state.social_status as u8);

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
        let specs =
            collect_profile_slot_specs(is_other_player, &profile_state.equipment, &eq_state.0);
        let slots = build_equipment_slots(asset_loader, &game_files, &specs);
        assign_equipment(&mut profile, slots);

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
    mut popup_manager: ResMut<crate::slint_support::popups::PopupManager>,
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
                popup_manager.close(crate::slint_support::popups::PopupId::Profile);

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
                if !popup_manager.is_open(crate::slint_support::popups::PopupId::Profile) {
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
        profile.social_status = u8_to_social_status(profile_state.social_status as u8);

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
        let specs =
            collect_profile_slot_specs(is_other_player, &profile_state.equipment, &eq_state.0);
        let slots = build_equipment_slots(asset_loader, &game_files, &specs);
        assign_equipment(&mut profile, slots);

        game_state.set_profile(profile);
        popup_manager.open(crate::slint_support::popups::PopupId::Profile);
        tracing::info!("Showing self profile panel");
    }
}
