//! Device wgpu + tiers de calidad por plataforma.
//! Potato (GTX600) / Mobile (Mali-G52) / Balanced (Xe/Vega/M1) / Ultra.

use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityTier {
    Potato,
    Mobile,
    Balanced,
    Ultra,
}

impl QualityTier {
    pub fn auto() -> Self {
        // Detección en runtime vía wgpu adapter info (simplificada aquí).
        // En PC discreta → Balanced; el autotuning dinámico ajusta después.
        Self::Balanced
    }
    pub fn render_scale(&self) -> f32 {
        match self {
            Self::Potato => 0.5,
            Self::Mobile => 0.65,
            Self::Balanced => 0.85,
            Self::Ultra => 1.0,
        }
    }
    pub fn gi_rays(&self) -> u32 {
        match self {
            Self::Potato => 0,
            Self::Mobile => 2,
            Self::Balanced => 4,
            Self::Ultra => 8,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub tier: QualityTier,
    pub vsync: bool,
}

pub struct RenderDevice {
    #[allow(dead_code)]
    instance: wgpu::Instance,
    #[allow(dead_code)]
    adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: RenderConfig,
}

impl RenderDevice {
    /// Inicialización headless (para tests/CI sin ventana).
    /// En la app real se usa `new_with_surface` (winit).
    pub async fn new_headless(tier: QualityTier) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("sin adaptador GPU"))?;
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("rivaren"),
                    required_features: wgpu::Features::empty(),
                    // Límites rebajados para Mali-G52 / GTX600.
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await?;
        Ok(Self {
            instance,
            adapter,
            device,
            queue,
            config: RenderConfig { tier, vsync: true },
        })
    }
}
