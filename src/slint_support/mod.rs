pub mod assets;
pub mod callbacks;
pub mod frame_exchange;
pub mod gpu_init;
pub mod input_bridge;
pub mod profile_bridge;
pub mod rendering_notifier;
pub mod state_bridge;

// Re-exports for convenience
pub use gpu_init::initialize_gpu_world;
pub use profile_bridge::{ShowSelfProfileEvent, handle_show_self_profile, sync_profile_to_slint};

use bevy::prelude::*;
use slint::ComponentHandle;
use slint::wgpu_28::{WGPUConfiguration, WGPUSettings};
use std::cell::RefCell;
use std::rc::Rc;

use crate::MainWindow;
use state_bridge::{SlintUiChannels, SlintWindow};

/// Marker that GPU + surface + scene/camera are ready for systems.
#[derive(Resource, Default)]
pub struct SlintGpuReady(pub bool);

/// Double-click event coordinates from Slint.
#[derive(Resource, Debug, Clone, Message)]
pub struct SlintDoubleClickEvent(pub f32, pub f32);

/// Attach Slint UI to the provided Bevy `App` and return the created `MainWindow`.
/// This consumes the App so the returned Slint notifier closure can own it and
/// drive updates from Slint's rendering callbacks.
pub fn attach_slint_ui(mut app: App) -> MainWindow {
    // Configure WGPU for Slint backend
    let mut wgpu_settings = WGPUSettings::default();
    wgpu_settings.device_required_features = wgpu::Features::IMMEDIATES;
    wgpu_settings.device_required_limits.max_immediate_size = 16;

    slint::BackendSelector::new()
        .require_wgpu_28(WGPUConfiguration::Automatic(wgpu_settings))
        .select()
        .expect("Unable to create Slint backend with WGPU based renderer");

    // Finish building schedules so systems are ready before Slint takes control.
    app.finish();
    app.cleanup();

    let slint_app = MainWindow::new().unwrap();

    // Set up input event queues
    let key_event_queue = input_bridge::new_shared_queue();
    let pointer_event_queue = input_bridge::new_shared_pointer_queue();
    let scroll_event_queue = input_bridge::new_shared_scroll_queue();
    let double_click_queue = input_bridge::new_shared_double_click_queue();

    app.insert_resource(input_bridge::SlintKeyEventQueue(key_event_queue.clone()));
    app.insert_resource(input_bridge::SlintPointerEventQueue(
        pointer_event_queue.clone(),
    ));
    app.insert_resource(input_bridge::SlintScrollEventQueue(
        scroll_event_queue.clone(),
    ));
    app.insert_resource(input_bridge::SlintDoubleClickQueue(
        double_click_queue.clone(),
    ));

    // Wire input callbacks
    callbacks::wire_input_callbacks(
        &slint_app,
        &key_event_queue,
        &pointer_event_queue,
        &scroll_event_queue,
        &double_click_queue,
    );

    // Install weak handle so Bevy systems can mutate properties
    app.world_mut()
        .insert_resource(SlintWindow(slint_app.as_weak()));

    // Wire UI callbacks -> crossbeam channel -> UiInbound messages
    if let Some(ch) = app.world().get_resource::<SlintUiChannels>() {
        callbacks::wire_login_callbacks(&slint_app, ch.tx.clone());
        callbacks::wire_game_callbacks(&slint_app, ch.tx.clone());
        callbacks::wire_settings_callbacks(&slint_app, ch.tx.clone());
    }

    // Set up rendering notifier
    let slint_app_handle = slint_app.as_weak();
    let app = Rc::new(RefCell::new(app));
    let last_update = Rc::new(RefCell::new(std::time::Instant::now()));

    let app_for_notifier = app.clone();
    let last_update_for_notifier = last_update.clone();

    slint_app
        .window()
        .set_rendering_notifier(move |rendering_state, graphics_api| {
            let mut app = app_for_notifier.borrow_mut();
            match rendering_state {
                slint::RenderingState::RenderingSetup => {
                    if let slint::GraphicsAPI::WGPU28 { device, queue, .. } = graphics_api {
                        let Some(strong) = slint_app_handle.upgrade() else {
                            return;
                        };

                        let window = strong.window();
                        gpu_init::initialize_gpu_world(
                            &mut app.world_mut(),
                            &device,
                            &queue,
                            window,
                            wgpu::TextureFormat::Rgba8Unorm,
                        );
                        let size = window.size();

                        rendering_notifier::seed_back_buffers(
                            &mut app,
                            &device,
                            size.width,
                            size.height,
                        );

                        tracing::info!("WGPU Rendering setup complete (Slint -> Bevy bridge)");

                        // One update so startup systems that depend on GPU can initialize.
                        app.update();
                        *last_update_for_notifier.borrow_mut() = std::time::Instant::now();
                    }
                }
                slint::RenderingState::BeforeRendering => {
                    app.update();
                    *last_update_for_notifier.borrow_mut() = std::time::Instant::now();

                    let Some(strong) = slint_app_handle.upgrade() else {
                        return;
                    };
                    strong.window().request_redraw();

                    rendering_notifier::handle_before_rendering(
                        &mut app,
                        &strong,
                        |w| w.get_requested_texture_width() as u32,
                        |w| w.get_requested_texture_height() as u32,
                        |w| w.get_texture_scale(),
                        |w, b| w.set_use_pixelated_filtering(b),
                        |w| w.get_texture(),
                        |w, img| w.set_texture(img),
                    );
                }
                _ => {}
            }
        })
        .expect("Failed to set rendering notifier - WGPU integration may not be available");

    // Background update timer: ensures Bevy keeps ticking (reading packets, etc) even when Slint
    // pauses rendering because the window is not visible.
    let app_for_timer = app.clone();
    let last_update_for_timer = last_update;
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(100),
        move || {
            if last_update_for_timer.borrow().elapsed() >= std::time::Duration::from_millis(100) {
                if let Ok(mut app) = app_for_timer.try_borrow_mut() {
                    app.update();
                    *last_update_for_timer.borrow_mut() = std::time::Instant::now();
                }
            }
        },
    );
    app.borrow_mut().insert_non_send_resource(timer);

    slint_app
}
