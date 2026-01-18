mod common;

use common::TestScene;
use packets::server::{self};

#[test]
fn test_player_on_map() {
    let mut scene = TestScene::new("assets/data.arx", "assets/maps");

    scene.load_map(498, 12, 12);

    let player_id = scene.next_entity_id();
    let player_id_2 = scene.next_entity_id();
    scene.display_player(server::display_player::DisplayPlayer {
        id: player_id,
        x: 6,
        y: 8,
        direction: 2,
        args: server::display_player::DisplayArgs::Normal {
            head_sprite: 5,
            body_sprite: 16,
            pants_color: 0,
            armor_sprite1: 4,
            boots_sprite: 1,
            armor_sprite2: 4,
            shield_sprite: 255,
            weapon_sprite: 6,
            head_color: 5,
            boots_color: 12,
            accessory_color1: 0,
            accessory_sprite1: 0,
            accessory_color2: 0,
            accessory_sprite2: 0,
            accessory_color3: 0,
            accessory_sprite3: 0,
            lantern_size: 0,
            rest_position: 0,
            overcoat_sprite: 0,
            overcoat_color: 0,
            body_color: 0,
            is_transparent: false,
            face_sprite: 0,
            is_male: true,
        },
        ..Default::default()
    });
    scene.display_player(server::display_player::DisplayPlayer {
        id: player_id_2,
        x: 8,
        y: 6,
        direction: 3,
        args: server::display_player::DisplayArgs::Normal {
            head_sprite: 4,
            body_sprite: 32,
            pants_color: 0,
            armor_sprite1: 1,
            boots_sprite: 1,
            armor_sprite2: 1,
            shield_sprite: 255,
            weapon_sprite: 0,
            head_color: 11,
            boots_color: 13,
            accessory_color1: 0,
            accessory_sprite1: 0,
            accessory_color2: 0,
            accessory_sprite2: 0,
            accessory_color3: 0,
            accessory_sprite3: 0,
            lantern_size: 0,
            rest_position: 0,
            overcoat_sprite: 0,
            overcoat_color: 0,
            body_color: 0,
            is_transparent: false,
            face_sprite: 0,
            is_male: false,
        },
        ..Default::default()
    });

    let item_id = scene.next_entity_id();
    scene.display_entities(server::DisplayVisibleEntities {
        entities: vec![server::EntityInfo::Item {
            x: 6,
            y: 8,
            id: item_id,
            sprite: 217,
            color: 0,
        }],
    });

    scene.set_light_level(packets::server::LightLevelKind::DarkestA);

    scene.update();
    scene.center_camera_on_tile(7.0, 7.0);
    scene.update();

    let png = scene.capture(240, 160);
    insta::assert_binary_snapshot!(".png", png);
}

#[test]
fn test_player_on_map_2() {
    let mut scene = TestScene::new("assets/data.arx", "assets/maps");

    scene.load_map(498, 12, 12);

    let player_id_1 = scene.next_entity_id();
    scene.display_player(server::display_player::DisplayPlayer {
        id: player_id_1,
        x: 6,
        y: 7,
        direction: 2,
        args: server::display_player::DisplayArgs::Normal {
            head_sprite: 146,
            body_sprite: 16,
            pants_color: 0,
            armor_sprite1: 265,
            boots_sprite: 1,
            armor_sprite2: 265,
            shield_sprite: 255,
            weapon_sprite: 0,
            head_color: 2,
            boots_color: 9,
            accessory_color1: 1,
            accessory_sprite1: 6,
            accessory_color2: 0,
            accessory_sprite2: 0,
            accessory_color3: 0,
            accessory_sprite3: 0,
            lantern_size: 0,
            rest_position: 0,
            overcoat_sprite: 0,
            overcoat_color: 0,
            body_color: 0,
            is_transparent: false,
            face_sprite: 0,
            is_male: true,
        },
        ..Default::default()
    });
    let player_id_2 = scene.next_entity_id();
    scene.display_player(server::display_player::DisplayPlayer {
        id: player_id_2,
        x: 5,
        y: 7,
        direction: 2,
        args: server::display_player::DisplayArgs::Normal {
            head_sprite: 356,
            body_sprite: 16,
            pants_color: 0,
            armor_sprite1: 364,
            boots_sprite: 14,
            armor_sprite2: 364,
            shield_sprite: 255,
            weapon_sprite: 90,
            head_color: 0,
            boots_color: 1,
            accessory_color1: 1,
            accessory_sprite1: 6,
            accessory_color2: 0,
            accessory_sprite2: 0,
            accessory_color3: 0,
            accessory_sprite3: 0,
            lantern_size: 0,
            rest_position: 0,
            overcoat_sprite: 332,
            overcoat_color: 0,
            body_color: 0,
            is_transparent: false,
            face_sprite: 0,
            is_male: true,
        },
        ..Default::default()
    });
    let player_id_3 = scene.next_entity_id();
    scene.display_player(server::display_player::DisplayPlayer {
        id: player_id_3,
        x: 5,
        y: 9,
        direction: 1,
        args: server::display_player::DisplayArgs::Normal {
            head_sprite: 32,
            body_sprite: 32,
            pants_color: 0,
            armor_sprite1: 361,
            boots_sprite: 239,
            armor_sprite2: 361,
            shield_sprite: 255,
            weapon_sprite: 186,
            head_color: 15,
            boots_color: 58,
            accessory_color1: 0,
            accessory_sprite1: 0,
            accessory_color2: 0,
            accessory_sprite2: 0,
            accessory_color3: 0,
            accessory_sprite3: 0,
            lantern_size: 0,
            rest_position: 0,
            overcoat_sprite: 0,
            overcoat_color: 0,
            body_color: 0,
            is_transparent: false,
            face_sprite: 0,
            is_male: false,
        },
        ..Default::default()
    });

    let item_id = scene.next_entity_id();
    let item_id_2 = scene.next_entity_id();
    let item_id_3 = scene.next_entity_id();
    let item_id_4 = scene.next_entity_id();
    scene.display_entities(server::DisplayVisibleEntities {
        entities: vec![
            server::EntityInfo::Item {
                x: 3,
                y: 7,
                id: item_id,
                sprite: 2005,
                color: 0,
            },
            server::EntityInfo::Item {
                x: 4,
                y: 7,
                id: item_id_2,
                sprite: 2036,
                color: 0,
            },
            server::EntityInfo::Item {
                x: 5,
                y: 7,
                id: item_id_3,
                sprite: 2005,
                color: 0,
            },
            server::EntityInfo::Item {
                x: 4,
                y: 8,
                id: item_id_4,
                sprite: 2030,
                color: 0,
            },
        ],
    });

    scene.set_light_level(packets::server::LightLevelKind::LightestA);

    scene.update();
    scene.center_camera_on_tile(5.0, 7.0);
    scene.update();

    let png = scene.capture(130, 100);
    insta::assert_binary_snapshot!(".png", png);
}

#[test]
fn test_player_on_map_3() {
    let mut scene = TestScene::new("assets/data.arx", "assets/maps");

    scene.load_map(505, 50, 50);

    let player_id_1 = scene.next_entity_id();
    scene.display_player(server::display_player::DisplayPlayer {
        id: player_id_1,
        x: 36,
        y: 44,
        direction: 2,
        args: server::display_player::DisplayArgs::Normal {
            head_sprite: 238,
            body_sprite: 16,
            pants_color: 0,
            armor_sprite1: 386,
            boots_sprite: 229,
            armor_sprite2: 386,
            shield_sprite: 255,
            weapon_sprite: 166,
            head_color: 16,
            boots_color: 58,
            accessory_color1: 0,
            accessory_sprite1: 165,
            accessory_color2: 0,
            accessory_sprite2: 0,
            accessory_color3: 0,
            accessory_sprite3: 0,
            lantern_size: 0,
            rest_position: 0,
            overcoat_sprite: 1065,
            overcoat_color: 1,
            body_color: 0,
            is_transparent: false,
            face_sprite: 0,
            is_male: true,
        },
        ..Default::default()
    });
    let player_id_2 = scene.next_entity_id();
    scene.display_player(server::display_player::DisplayPlayer {
        id: player_id_2,
        x: 37,
        y: 44,
        direction: 2,
        args: server::display_player::DisplayArgs::Normal {
            head_sprite: 240,
            body_sprite: 32,
            pants_color: 0,
            armor_sprite1: 315,
            boots_sprite: 5,
            armor_sprite2: 315,
            shield_sprite: 255,
            weapon_sprite: 26,
            head_color: 22,
            boots_color: 0,
            accessory_color1: 1,
            accessory_sprite1: 47,
            accessory_color2: 0,
            accessory_sprite2: 0,
            accessory_color3: 0,
            accessory_sprite3: 0,
            lantern_size: 0,
            rest_position: 0,
            overcoat_sprite: 1065,
            overcoat_color: 0,
            body_color: 0,
            is_transparent: false,
            face_sprite: 0,
            is_male: false,
        },
        ..Default::default()
    });
    let player_id_3 = scene.next_entity_id();
    scene.display_player(server::display_player::DisplayPlayer {
        id: player_id_3,
        x: 38,
        y: 44,
        direction: 2,
        args: server::display_player::DisplayArgs::Normal {
            head_sprite: 459,
            body_sprite: 32,
            pants_color: 0,
            armor_sprite1: 362,
            boots_sprite: 239,
            armor_sprite2: 362,
            shield_sprite: 23,
            weapon_sprite: 275,
            head_color: 15,
            boots_color: 58,
            accessory_color1: 0,
            accessory_sprite1: 50,
            accessory_color2: 0,
            accessory_sprite2: 0,
            accessory_color3: 0,
            accessory_sprite3: 0,
            lantern_size: 0,
            rest_position: 0,
            overcoat_sprite: 1065,
            overcoat_color: 0,
            body_color: 6,
            is_transparent: false,
            face_sprite: 0,
            is_male: false,
        },
        ..Default::default()
    });
    let player_id_4 = scene.next_entity_id();
    scene.display_player(server::display_player::DisplayPlayer {
        id: player_id_4,
        x: 39,
        y: 44,
        direction: 2,
        args: server::display_player::DisplayArgs::Normal {
            head_sprite: 238,
            body_sprite: 32,
            pants_color: 0,
            armor_sprite1: 388,
            boots_sprite: 239,
            armor_sprite2: 388,
            shield_sprite: 255,
            weapon_sprite: 276,
            head_color: 15,
            boots_color: 58,
            accessory_color1: 0,
            accessory_sprite1: 255,
            accessory_color2: 0,
            accessory_sprite2: 0,
            accessory_color3: 0,
            accessory_sprite3: 0,
            lantern_size: 0,
            rest_position: 0,
            overcoat_sprite: 1057,
            overcoat_color: 1,
            body_color: 0,
            is_transparent: false,
            face_sprite: 0,
            is_male: false,
        },
        ..Default::default()
    });
    let player_id_5 = scene.next_entity_id();
    scene.display_player(server::display_player::DisplayPlayer {
        id: player_id_5,
        x: 40,
        y: 44,
        direction: 2,
        args: server::display_player::DisplayArgs::Normal {
            head_sprite: 251,
            body_sprite: 32,
            pants_color: 0,
            armor_sprite1: 386,
            boots_sprite: 229,
            armor_sprite2: 386,
            shield_sprite: 255,
            weapon_sprite: 142,
            head_color: 30,
            boots_color: 58,
            accessory_color1: 1,
            accessory_sprite1: 277,
            accessory_color2: 0,
            accessory_sprite2: 0,
            accessory_color3: 0,
            accessory_sprite3: 0,
            lantern_size: 0,
            rest_position: 0,
            overcoat_sprite: 1071,
            overcoat_color: 0,
            body_color: 5,
            is_transparent: false,
            face_sprite: 0,
            is_male: false,
        },
        ..Default::default()
    });

    scene.update();
    scene.center_camera_on_tile(38.0, 44.0);
    scene.update();

    let png = scene.capture(180, 180);
    insta::assert_binary_snapshot!(".png", png);
}

#[test]
fn test_player_on_map_4() {
    let mut scene = TestScene::new("assets/data.arx", "assets/maps");

    scene.load_map(505, 50, 50);

    let player_id_1 = scene.next_entity_id();
    scene.display_player(server::display_player::DisplayPlayer {
        id: player_id_1,
        x: 36,
        y: 44,
        direction: 2,
        args: server::display_player::DisplayArgs::Normal {
            head_sprite: 150,
            body_sprite: 16,
            pants_color: 0,
            armor_sprite1: 283,
            boots_sprite: 235,
            armor_sprite2: 283,
            shield_sprite: 255,
            weapon_sprite: 186,
            head_color: 1,
            boots_color: 20,
            accessory_color1: 0,
            accessory_sprite1: 0,
            accessory_color2: 0,
            accessory_sprite2: 0,
            accessory_color3: 0,
            accessory_sprite3: 0,
            lantern_size: 0,
            rest_position: 0,
            overcoat_sprite: 0,
            overcoat_color: 0,
            body_color: 5,
            is_transparent: false,
            face_sprite: 0,
            is_male: true,
        },
        ..Default::default()
    });
    let player_id_2 = scene.next_entity_id();
    scene.display_player(server::display_player::DisplayPlayer {
        id: player_id_2,
        x: 37,
        y: 44,
        direction: 2,
        args: server::display_player::DisplayArgs::Normal {
            head_sprite: 392,
            body_sprite: 32,
            pants_color: 0,
            armor_sprite1: 156,
            boots_sprite: 5,
            armor_sprite2: 156,
            shield_sprite: 255,
            weapon_sprite: 255,
            head_color: 1,
            boots_color: 0,
            accessory_color1: 1,
            accessory_sprite1: 199,
            accessory_color2: 0,
            accessory_sprite2: 0,
            accessory_color3: 0,
            accessory_sprite3: 0,
            lantern_size: 0,
            rest_position: 0,
            overcoat_sprite: 1013,
            overcoat_color: 1,
            body_color: 0,
            is_transparent: false,
            face_sprite: 0,
            is_male: false,
        },
        ..Default::default()
    });
    let player_id_3 = scene.next_entity_id();
    scene.display_player(server::display_player::DisplayPlayer {
        id: player_id_3,
        x: 38,
        y: 44,
        direction: 2,
        args: server::display_player::DisplayArgs::Normal {
            head_sprite: 252,
            body_sprite: 16,
            pants_color: 0,
            armor_sprite1: 386,
            boots_sprite: 237,
            armor_sprite2: 386,
            shield_sprite: 255,
            weapon_sprite: 175,
            head_color: 2,
            boots_color: 15,
            accessory_color1: 0,
            accessory_sprite1: 0,
            accessory_color2: 0,
            accessory_sprite2: 0,
            accessory_color3: 0,
            accessory_sprite3: 0,
            lantern_size: 0,
            rest_position: 0,
            overcoat_sprite: 0,
            overcoat_color: 0,
            body_color: 0,
            is_transparent: false,
            face_sprite: 0,
            is_male: true,
        },
        ..Default::default()
    });

    scene.update();
    scene.center_camera_on_tile(37.0, 44.0);
    scene.update();

    let png = scene.capture(180, 180);
    insta::assert_binary_snapshot!(".png", png);
}

#[test]
fn test_player_movement() {
    let mut scene = TestScene::new("assets/data.arx", "assets/maps");

    scene.load_map(498, 12, 12);

    let player_id = scene.next_entity_id();
    scene.set_local_player_id(player_id);

    scene.display_player(server::display_player::DisplayPlayer {
        id: player_id,
        x: 6,
        y: 8,
        direction: 2,
        args: server::display_player::DisplayArgs::Normal {
            head_sprite: 5,
            body_sprite: 16,
            pants_color: 0,
            armor_sprite1: 4,
            boots_sprite: 1,
            armor_sprite2: 4,
            shield_sprite: 255,
            weapon_sprite: 6,
            head_color: 5,
            boots_color: 12,
            accessory_color1: 0,
            accessory_sprite1: 0,
            accessory_color2: 0,
            accessory_sprite2: 0,
            accessory_color3: 0,
            accessory_sprite3: 0,
            lantern_size: 0,
            rest_position: 0,
            overcoat_sprite: 0,
            overcoat_color: 0,
            body_color: 0,
            is_transparent: false,
            face_sprite: 0,
            is_male: true,
        },
        ..Default::default()
    });

    // Initial update to spawn entities
    scene.update();
    // Center camera on player
    scene.center_camera_on_tile(6.0, 8.0);
    scene.update();

    // Take a step down (direction 2)
    scene.send_player_action(talgonite::events::PlayerAction::Walk(2));

    // Process the action (inserts the tween)
    scene.update();

    // Advance time by 250ms (halfway through the 500ms tween)
    scene.advance_time(std::time::Duration::from_millis(300));

    let png = scene.capture(240, 160);
    insta::assert_binary_snapshot!("player_movement_halfway.png", png);

    // Advance time by another 250ms (finish the tween)
    scene.advance_time(std::time::Duration::from_millis(300));

    let png = scene.capture(240, 160);
    insta::assert_binary_snapshot!("player_movement_finished.png", png);
}
