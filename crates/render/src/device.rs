//! Contexto GPU (wgpu) para ventana y headless.
//! wgpu 29: Vulkan first-class en Linux/Windows/Android, Metal en macOS, DX12 en Windows.

use anyhow::{Context, Result};
use std::sync::Arc;

pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: Option<wgpu::Surface<'static>>,
    pub surface_config: Option<wgpu::SurfaceConfiguration>,
    pub info: wgpu::AdapterInfo,
    pub format: wgpu::TextureFormat,
}

impl GpuContext {
    /// Crea contexto con ventana (surface) o headless (tests/CI).
    pub async fn new(
        window: Option<Arc<winit::window::Window>>,
        size: (u32, u32),
        for_present: bool,
    ) -> Result<Self> {
        let mut iid = wgpu::InstanceDescriptor::new_without_display_handle();
        iid.backends = wgpu::Backends::all();
        let instance = wgpu::Instance::new(iid);
        let surface = match window {
            Some(w) => Some(
                instance
                    .create_surface(w)
                    .context("create_surface falló")?,
            ),
            None => None,
        };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: surface.as_ref(),
                force_fallback_adapter: false,
            })
            .await
            .context("sin adaptador GPU (drivers Vulkan/Metal/DX12)")?;
        let info = adapter.get_info();
        // Límites downlevel: GTX600 / Mali-G52 / Vulkan 1.1 / GLES3.
        let limits = wgpu::Limits::downlevel_defaults();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("rivaren"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                experimental_features: Default::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .context("request_device falló")?;
        let (surface_config, format) = if let Some(s) = &surface {
            let caps = s.get_capabilities(&adapter);
            let format = caps
                .formats
                .iter()
                .copied()
                .find(|f| f.is_srgb())
                .unwrap_or(caps.formats[0]);
            let cfg = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                width: size.0.max(1),
                height: size.1.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            s.configure(&device, &cfg);
            (Some(cfg), format)
        } else {
            (None, wgpu::TextureFormat::Rgba16Float)
        };
        tracing::info!(
            "GPU: {} [{:?}] backend={:?} driver={}",
            info.name,
            info.device_type,
            info.backend,
            info.driver
        );
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            surface,
            surface_config,
            info,
            format,
        })
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if let (Some(s), Some(cfg)) = (&self.surface, &mut self.surface_config) {
            cfg.width = w.max(1);
            cfg.height = h.max(1);
            s.configure(&self.device, cfg);
        }
    }

    pub fn size(&self) -> (u32, u32) {
        self.surface_config
            .as_ref()
            .map(|c| (c.width, c.height))
            .unwrap_or((1280, 720))
    }
}
