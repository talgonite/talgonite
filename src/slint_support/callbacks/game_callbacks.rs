//! Game-related callback wiring for Slint UI.

use crossbeam_channel::Sender;
use slint::ComponentHandle;

use crate::webui::ipc::{UiToCore, WorldListFilter};
use game_ui::slint_types::GoldDropState;

use crate::{DragDropState, GameState, MainWindow, NpcDialogState, SlotPanelType};

/// Convert Slint SlotPanelType to game types.
fn slint_to_game_panel(panel: SlotPanelType) -> game_types::SlotPanelType {
    match panel {
        SlotPanelType::Item => game_types::SlotPanelType::Item,
        SlotPanelType::Gold => game_types::SlotPanelType::Gold,
        SlotPanelType::Skill => game_types::SlotPanelType::Skill,
        SlotPanelType::Spell => game_types::SlotPanelType::Spell,
        SlotPanelType::Hotbar => game_types::SlotPanelType::Hotbar,
        SlotPanelType::World => game_types::SlotPanelType::World,
        SlotPanelType::None => game_types::SlotPanelType::None,
    }
}

/// Wire all game-related callbacks: world map, menu, chat, equipment, hotbar, drag-drop.
pub fn wire_game_callbacks(slint_app: &MainWindow, tx: Sender<UiToCore>) {
    let game_state = slint_app.global::<GameState>();

    // World map click
    {
        let tx = tx.clone();
        game_state.on_world_map_click(move |map_id, x, y, check_sum| {
            let _ = tx.send(UiToCore::WorldMapClick {
                map_id: map_id as u16,
                x: x as u16,
                y: y as u16,
                check_sum: check_sum as u16,
            });
        });
    }

    // NPC Dialog callbacks
    let npc_dialog = slint_app.global::<NpcDialogState>();

    // Gold drop prompt callbacks
    let gold_drop = slint_app.global::<GoldDropState>();
    {
        let tx = tx.clone();
        gold_drop.on_submit(move |amount: slint::SharedString| {
            let _ = tx.send(UiToCore::GoldDropSubmit {
                amount: amount.to_string(),
            });
        });
    }
    {
        let tx = tx.clone();
        gold_drop.on_cancel(move || {
            let _ = tx.send(UiToCore::GoldDropCancel);
        });
    }

    // Menu select (option selection)
    {
        let tx = tx.clone();
        npc_dialog.on_select_option_request(move |id, name: slint::SharedString| {
            let _ = tx.send(UiToCore::MenuSelect {
                id,
                name: name.to_string(),
            });
        });
    }

    // Close dialog
    {
        let tx = tx.clone();
        npc_dialog.on_close_request(move || {
            let _ = tx.send(UiToCore::MenuClose);
        });
    }

    // Text entry submission
    {
        let tx = tx.clone();
        npc_dialog.on_submit_text_request(move |text: slint::SharedString| {
            let _ = tx.send(UiToCore::MenuSelect {
                id: 0,
                name: text.to_string(),
            });
        });
    }

    // Unequip
    {
        let tx = tx.clone();
        game_state.on_unequip(move |slot| {
            if tx.send(UiToCore::Unequip { slot: slot as u8 }).is_err() {
                tracing::error!("Failed to send Unequip message");
            }
        });
    }

    // Use action
    {
        let tx = tx.clone();
        game_state.on_use_action(move |panel, slot| {
            if tx
                .send(UiToCore::ActivateAction {
                    category: slint_to_game_panel(panel),
                    index: slot as usize,
                })
                .is_err()
            {
                tracing::error!("Failed to send ActivateAction message");
            }
        });
    }

    // Set hotbar panel
    {
        let tx = tx.clone();
        game_state.on_set_hotbar_panel(move |panel_num| {
            if tx
                .send(UiToCore::SetHotbarPanel {
                    panel_num: panel_num as u8,
                })
                .is_err()
            {
                tracing::error!("Failed to send SetHotbarPanel message");
            }
        });
    }

    // Refresh world list
    {
        let tx = tx.clone();
        game_state.on_refresh_world_list(move || {
            let _ = tx.send(UiToCore::RequestWorldList);
        });
    }

    // Set world list filter
    {
        let tx = tx.clone();
        game_state.on_set_world_list_filter(move |class, master_only, search| {
            let _ = tx.send(UiToCore::SetWorldListFilter {
                filter: WorldListFilter {
                    class: if class == "All" {
                        None
                    } else {
                        Some(class.to_string())
                    },
                    master_only,
                    search: search.to_string(),
                },
            });
        });
    }

    // Send chat
    {
        let tx = tx.clone();
        game_state.on_send_chat(move |text| {
            if tx
                .send(UiToCore::ChatSubmit {
                    mode: "all".to_string(),
                    text: text.to_string(),
                    target: None,
                })
                .is_err()
            {
                tracing::error!("Failed to send ChatSubmit message");
            }
        });
    }

    // Send whisper
    {
        let tx = tx.clone();
        let slint_app_weak = slint_app.as_weak();
        game_state.on_send_whisper(move |target, text| {
            if let Some(app) = slint_app_weak.upgrade() {
                let gs = app.global::<GameState>();
                gs.set_last_whisper_target(target.clone());
            }

            if tx
                .send(UiToCore::ChatSubmit {
                    mode: "whisper".to_string(),
                    text: text.to_string(),
                    target: Some(target.to_string()),
                })
                .is_err()
            {
                tracing::error!("Failed to send ChatSubmit (whisper) message");
            }
        });
    }

    // Drag-drop action
    let dragdrop_state = slint_app.global::<DragDropState>();
    {
        let tx = tx.clone();
        dragdrop_state.on_action_drag_drop(
            move |src_panel, src_slot, dst_panel, dst_slot, x, y| {
                tracing::info!(
                    "DragDropAction from {:?} slot {} to {:?} slot {} at ({}, {})",
                    src_panel,
                    src_slot,
                    dst_panel,
                    dst_slot,
                    x,
                    y
                );

                if tx
                    .send(UiToCore::DragDropAction {
                        src_category: slint_to_game_panel(src_panel),
                        src_index: src_slot as usize,
                        dst_category: slint_to_game_panel(dst_panel),
                        dst_index: dst_slot as usize,
                        x,
                        y,
                    })
                    .is_err()
                {
                    tracing::error!("Failed to send DragDropAction message");
                }
            },
        );
    }
}
