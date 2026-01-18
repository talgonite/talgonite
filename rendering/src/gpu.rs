use glam::UVec2;
use wgpu;

/// Common GPU initialization for both desktop and web
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_format: wgpu::TextureFormat,
}

impl GpuContext {
    /// Initialize GPU context for web (WebGL)
    #[cfg(target_arch = "wasm32")]
    pub async fn new_web(canvas_id: &str) -> Result<Self, Box<dyn std::error::Error>> {
        use wasm_bindgen::JsCast;
        use web_sys::{HtmlCanvasElement, Window};

        let window = web_sys::window().ok_or("No global window")?;
        let document = window.document().ok_or("No document")?;
        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or("Canvas not found")?
            .dyn_into::<HtmlCanvasElement>()?;

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        let surface = instance.create_surface_from_canvas(&canvas)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or("Failed to find adapter")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("WebDemo Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await?;

        let capabilities = surface.get_capabilities(&adapter);
        let surface_format = capabilities
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(capabilities.formats[0]);

        // Configure the surface
        let size = UVec2::new(canvas.width(), canvas.height());
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.x,
            height: size.y,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Ok(GpuContext {
            device,
            queue,
            surface_format,
        })
    }

    /// Initialize GPU context for desktop
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new_desktop(
        instance: &wgpu::Instance,
        surface: &wgpu::Surface<'static>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let adapter = wgpu::util::initialize_adapter_from_env_or_default(instance, Some(surface))
            .await
            .ok_or("Failed to find adapter")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Desktop Device"),
                    required_features: wgpu::Features::default(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await?;

        let capabilities = surface.get_capabilities(&adapter);
        let surface_format = capabilities
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(capabilities.formats[0]);

        Ok(GpuContext {
            device,
            queue,
            surface_format,
        })
    }
}
