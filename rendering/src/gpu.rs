use wgpu;

/// Common GPU initialization
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface_format: wgpu::TextureFormat,
}

impl GpuContext {
    /// Initialize GPU context for desktop
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
