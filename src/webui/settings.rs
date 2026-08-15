use bevy::prelude::*;

use crate::input::{InputBindings, UnifiedInputBindings};
use crate::settings_types::{GraphicsSettings, Settings as SettingsFile};
use crate::webui::plugin::UiOutbound;

macro_rules! for_each_binding {
    ($mac:ident) => {
        $mac!(move_up);
        $mac!(move_down);
        $mac!(move_left);
        $mac!(move_right);
        $mac!(inventory);
        $mac!(skills);
        $mac!(spells);
        $mac!(settings);
        $mac!(refresh);
        $mac!(toggle_overview);
        $mac!(basic_attack);
        $mac!(auto_attack_toggle);
        $mac!(item_pickup_below);
        $mac!(hotbar_slot_1);
        $mac!(hotbar_slot_2);
        $mac!(hotbar_slot_3);
        $mac!(hotbar_slot_4);
        $mac!(hotbar_slot_5);
        $mac!(hotbar_slot_6);
        $mac!(hotbar_slot_7);
        $mac!(hotbar_slot_8);
        $mac!(hotbar_slot_9);
        $mac!(hotbar_slot_10);
        $mac!(hotbar_slot_11);
        $mac!(hotbar_slot_12);
        $mac!(hotbar_slot_13);
        $mac!(hotbar_slot_14);
        $mac!(hotbar_slot_15);
        $mac!(hotbar_slot_16);
        $mac!(hotbar_slot_17);
        $mac!(hotbar_slot_18);
        $mac!(hotbar_slot_19);
        $mac!(hotbar_slot_20);
        $mac!(hotbar_slot_21);
        $mac!(hotbar_slot_22);
        $mac!(hotbar_slot_23);
        $mac!(hotbar_slot_24);
        $mac!(hotbar_slot_25);
        $mac!(hotbar_slot_26);
        $mac!(hotbar_slot_27);
        $mac!(hotbar_slot_28);
        $mac!(hotbar_slot_29);
        $mac!(hotbar_slot_30);
        $mac!(hotbar_slot_31);
        $mac!(hotbar_slot_32);
        $mac!(hotbar_slot_33);
        $mac!(hotbar_slot_34);
        $mac!(hotbar_slot_35);
        $mac!(hotbar_slot_36);
        $mac!(hotbar_slot_37);
        $mac!(hotbar_slot_38);
        $mac!(hotbar_slot_39);
        $mac!(hotbar_slot_40);
        $mac!(hotbar_slot_41);
        $mac!(hotbar_slot_42);
        $mac!(hotbar_slot_43);
        $mac!(hotbar_slot_44);
        $mac!(hotbar_slot_45);
        $mac!(hotbar_slot_46);
        $mac!(hotbar_slot_47);
        $mac!(hotbar_slot_48);
        $mac!(switch_to_inventory);
        $mac!(switch_to_skills);
        $mac!(switch_to_spells);
        $mac!(switch_to_hotbar_1);
        $mac!(switch_to_hotbar_2);
        $mac!(switch_to_hotbar_3);
    };
}

pub fn apply_rebind_key(
    action: &str,
    new_key: &str,
    index: usize,
    settings: &mut SettingsFile,
    input_bindings: &mut InputBindings,
    unified_bindings: &mut UnifiedInputBindings,
) {
    macro_rules! clear_conflicts {
        ($field:ident) => {
            for binding in settings.key_bindings.$field.iter_mut() {
                if !new_key.is_empty() && binding == new_key {
                    *binding = String::new();
                }
            }
        };
    }
    macro_rules! set_binding {
        ($field:ident) => {
            if action == stringify!($field) {
                settings.key_bindings.$field[index] = new_key.to_string();
            }
        };
    }
    for_each_binding!(clear_conflicts);
    for_each_binding!(set_binding);

    *unified_bindings = UnifiedInputBindings::from_settings(&settings.key_bindings);
    *input_bindings = InputBindings::from_settings(&settings.key_bindings);
}

pub fn apply_unbind_key(
    action: &str,
    index: usize,
    settings: &mut SettingsFile,
    input_bindings: &mut InputBindings,
    unified_bindings: &mut UnifiedInputBindings,
) {
    macro_rules! clear_binding {
        ($field:ident) => {
            if action == stringify!($field) {
                settings.key_bindings.$field[index] = String::new();
            }
        };
    }
    for_each_binding!(clear_binding);

    *unified_bindings = UnifiedInputBindings::from_settings(&settings.key_bindings);
    *input_bindings = InputBindings::from_settings(&settings.key_bindings);
}

pub fn apply_settings_change(xray_size: u8, settings: &mut SettingsFile) {
    settings.graphics.xray_size = crate::settings_types::XRaySize::from_u8(xray_size);
}

pub fn apply_volume_change(sfx: Option<f32>, music: Option<f32>, settings: &mut SettingsFile) {
    if let Some(sfx_vol) = sfx {
        settings.audio.sfx_volume = sfx_vol;
    }
    if let Some(music_vol) = music {
        settings.audio.music_volume = music_vol;
    }
}

pub fn apply_scale_input_change(progress: f32, settings: &mut SettingsFile) -> f32 {
    let scale = GraphicsSettings::scale_from_progress(progress);
    settings.graphics.scale = scale;
    scale
}

pub fn apply_modifier_rows_change(enabled: bool, settings: &mut SettingsFile) {
    settings.gameplay.modifier_hotbar_rows_target_custom_only = enabled;
}

pub fn write_snapshot_and_sync(outbound: &mut MessageWriter<UiOutbound>, settings: &SettingsFile) {
    outbound.write(UiOutbound(settings.to_snapshot_message(None)));
    outbound.write(UiOutbound(settings.to_sync_message()));
}

pub fn sync_settings_to_ui(settings: Res<SettingsFile>, mut outbound: MessageWriter<UiOutbound>) {
    if settings.is_changed() {
        outbound.write(UiOutbound(settings.to_sync_message()));
    }
}
