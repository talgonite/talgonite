//! Debug console window: live per-frame metrics plus an event log.

use bevy::prelude::*;
use slint::{ComponentHandle, Model};
use std::time::{Duration, Instant};

use crate::app_state::AppState;
use crate::resources::{DebugLog, FrameMetrics};

/// Weak handle to the debug console window, present only when it was shown.
#[derive(Resource)]
pub struct DebugConsoleWindow(pub slint::Weak<crate::DebugConsole>);

/// The metrics model stays alive between refreshes so rows keep their identity
/// and hover/tooltip state; replacing the model would recreate every row.
/// Rc-based (not Send), so this is a non-send resource.
pub struct ConsoleMetricsModel(pub Option<std::rc::Rc<slint::VecModel<crate::DebugMetric>>>);

const UPDATE_INTERVAL: Duration = Duration::from_millis(250);
const MAX_LOG_LINES: usize = 200;

/// Creates and shows the console window, returning a resource handle for the
/// Bevy side to update. Compiled in only with the `debug` cargo feature.
pub fn spawn_debug_console(console: crate::DebugConsole) -> DebugConsoleWindow {
    let _ = console.show();
    DebugConsoleWindow(console.as_weak())
}

pub fn update_debug_console(
    win: Res<DebugConsoleWindow>,
    metrics: Res<FrameMetrics>,
    mut log: ResMut<DebugLog>,
    mut lines: Local<Vec<String>>,
    mut last_update: Local<Option<Instant>>,
    state: Option<Res<State<AppState>>>,
    mut console_model: NonSendMut<ConsoleMetricsModel>,
) {
    let now = Instant::now();
    if last_update.is_some_and(|t| now.duration_since(t) < UPDATE_INTERVAL) {
        return;
    }
    *last_update = Some(now);

    let Some(console) = win.0.upgrade() else {
        return;
    };

    let top_systems_value = if metrics.last_top_systems.is_empty() {
        format!("{:.2} ms (no data)", metrics.last_update_us as f32 / 1000.0)
    } else {
        let systems = metrics
            .last_top_systems
            .iter()
            .map(|(name, us)| format!("{} {}us", short_system_name(name), us))
            .collect::<Vec<_>>()
            .join(" | ");
        format!(
            "{:.2} ms: {}",
            metrics.last_update_us as f32 / 1000.0,
            systems
        )
    };

    let rows: Vec<crate::DebugMetric> = vec![
        metric(
            "App state",
            &state
                .as_ref()
                .map_or_else(|| "n/a".to_string(), |s| format!("{:?}", s.get())),
            "Current Bevy app state.\nThe update-time breakdown and most other metrics only update while InGame.",
        ),
        metric(
            "Frame",
            &format!("#{}", metrics.frame_count),
            "Monotonic counter of Bevy updates:\none per rendered frame, plus ~10/s background timer ticks when rendering is paused.",
        ),
        metric(
            "FPS",
            &format!("{:.1}", metrics.last_fps),
            "1 / last ECS update duration.\nIncludes draw_frame while in game.",
        ),
        metric(
            "ECS update",
            &format!("{:.2} ms", metrics.last_update_us as f32 / 1000.0),
            "Full Bevy app.update() time, driven by Slint's BeforeRendering callback each frame.\nIncludes draw_frame.",
        ),
        metric(
            "Top systems",
            &top_systems_value,
            "Top Bevy systems by CPU time this frame (microseconds), captured from the trace feature's per-system spans.\nAnything not listed is spread across the remaining systems.",
        ),
        metric(
            "Draw",
            &format!(
                "{:.2} ms, {} passes, {} submit(s)",
                metrics.last_draw_us as f32 / 1000.0,
                metrics.last_draw_passes,
                metrics.last_queue_submits
            ),
            "draw_frame only: scene render passes into the game texture plus the wgpu submit.\nPasses counts world, translucent, composite, darkness, weather and minimap passes that actually ran.",
        ),
        metric(
            "Scene instances",
            &format!(
                "map {}, sprites {}, effects {}, weather {}",
                metrics.last_map_instances,
                metrics.last_sprite_instances,
                metrics.last_effect_instances,
                metrics.last_weather_instances
            ),
            "Live instances in the map renderer, unified sprite batch\n(players/creatures/items), effect manager and weather renderer.",
        ),
        metric(
            "Minimap",
            &format!(
                "{} tiles, {} markers",
                metrics.last_minimap_tiles, metrics.last_minimap_markers
            ),
            "Live minimap tile and marker instance counts.",
        ),
        metric(
            "Instance writes",
            &format!(
                "{} writes ({} updates, {} dedup-skipped)",
                metrics.last_instance_writes,
                metrics.last_instance_updates,
                metrics.last_instance_dedup_skips
            ),
            "Shared instance-buffer writes this frame (sprites, creatures, items,\neffects and minimap markers; map animation, weather and camera\nuniforms use other buffers).\nupdates = every update() call; dedup-skipped = identical values not re-uploaded.\nWrites are deferred and uploaded once per frame through a persistent staging belt.\nSmall nonzero writes are normal while sprites idle-animate, since frames advance.",
        ),
        metric(
            "Instance adds/removes",
            &format!(
                "{} / {}",
                metrics.last_instance_adds, metrics.last_instance_removes
            ),
            "Instance slot additions/removals, i.e. entities appearing or despawning this frame.",
        ),
        metric(
            "Slint property sets",
            &format!(
                "{} sent / {} attempted ({} saved)",
                metrics.last_slint_sets_sent,
                metrics.last_slint_sets_attempted,
                metrics
                    .last_slint_sets_attempted
                    .saturating_sub(metrics.last_slint_sets_sent)
            ),
            "Per-frame Slint setters from the world-label sync.\nattempted = value groups evaluated\nsent = setters actually called because the value changed\nsaved = attempted - sent",
        ),
        metric(
            "Slint model rebuilds",
            &format!("{}", metrics.last_slint_model_rebuilds),
            "Times the world-label and speech-bubble models were recreated this frame.\nShould stay 0 unless labels, bubbles or HP bars changed.",
        ),
        metric(
            "Slint core syncs fired",
            &format!("{}", metrics.last_slint_core_syncs),
            "apply_core_to_slint branches where is_changed() was true\n(inventory, abilities, hotbar, world list).\nPersistently high while idle means some system mutates those resources every frame.",
        ),
        metric(
            "Texture handoffs / repaints",
            &format!(
                "{} / {}",
                metrics.last_texture_handoffs, metrics.last_repaints
            ),
            "handoffs = game textures published to Slint\nrepaints = Slint BeforeRendering callbacks\nBoth stay ~1/frame while in game: the fixed cost of the always-render loop.",
        ),
    ];
    let model = match console_model.0.as_ref() {
        Some(model) => model.clone(),
        None => {
            let model = std::rc::Rc::new(slint::VecModel::<crate::DebugMetric>::default());
            for _ in 0..rows.len() {
                model.push(crate::DebugMetric::default());
            }
            console.set_metrics(slint::ModelRc::new(model.clone()));
            console_model.0 = Some(model.clone());
            model
        }
    };
    for (idx, row) in rows.into_iter().enumerate() {
        if model.row_data(idx) != Some(row.clone()) {
            model.set_row_data(idx, row);
        }
    }

    for line in log.drain() {
        if lines.len() >= MAX_LOG_LINES {
            lines.remove(0);
        }
        lines.push(line);
    }
    if !lines.is_empty() {
        console.set_log_text(slint::SharedString::from(lines.join("\n")));
    }
}

fn short_system_name(name: &str) -> &str {
    name.rsplit("::").next().unwrap_or(name)
}

fn metric(name: &str, value: &str, help: &str) -> crate::DebugMetric {
    crate::DebugMetric {
        name: slint::SharedString::from(name),
        value: slint::SharedString::from(value),
        help: slint::SharedString::from(help),
    }
}
