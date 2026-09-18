//! `rivaren-render`: motor gráfico GPU-driven sobre wgpu 29.
//!
//! Pipeline de frame:
//!   sombras CSM → cielo → terreno → agua → bloom → tonemap+TAA → upscale → UI.
//!
//! Todo Vulkan/DX12/Metal vía wgpu, con límites downlevel (GTX600 / Mali-G52).
//! Culling por frustum en CPU; MDI/HZB en G8.

pub mod camera;
pub mod device;
pub mod ui_batch;

pub use camera::{Camera, aabb_in_frustum, sphere_in_frustum};

/// Nombre del backend activo (informativo).
pub const GPU_BACKEND_NAME: &str = "wgpu29";
pub use device::GpuContext;

use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use glam::{IVec3, Mat4, Vec3, Vec4};
use std::collections::HashMap;

const SHADOW_RES: u32 = 2048;
pub const CASCADES: usize = 3;
const MAX_CHUNKS: usize = 8192;
const BLOOM_DIV: u32 = 4;

// ── Uniforms ─────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    prev_view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    cam_pos: [f32; 4],
    sun_dir: [f32; 4],
    sky_color: [f32; 4],
    horizon_color: [f32; 4],
    cascade_mats: [[[f32; 4]; 4]; CASCADES],
    cascade_splits: [f32; 4],
    misc: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct ChunkInfo {
    pub offset: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PostParams {
    texel: [f32; 2],
    bloom_strength: f32,
    exposure: f32,
    taa_blend: f32,
    frame: f32,
    underwater: f32,
    sharpen: f32,
    _pad: f32,
    _pad2: f32,
}

/// Instancia de entidad (caja) para el pase de entidades.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct EntityInstance {
    /// Centro en el mundo.
    pub pos: [f32; 3],
    pub _pad0: f32,
    /// Tamaño de la caja (ancho, alto, fondo).
    pub size: [f32; 3],
    pub _pad1: f32,
    /// Color RGB + alpha.
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ScreenParams {
    w: f32,
    h: f32,
    scale: f32,
    _pad: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QualityTier {
    Potato,
    Mobile,
    Balanced,
    Ultra,
}

impl QualityTier {
    pub fn auto() -> Self {
        Self::Balanced
    }
    pub fn render_scale(&self) -> f32 {
        match self {
            Self::Potato => 0.5,
            Self::Mobile => 0.65,
            Self::Balanced => 1.0,
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
    pub fn shadows(&self) -> usize {
        match self {
            Self::Potato => 1,
            Self::Mobile => 2,
            _ => CASCADES,
        }
    }
}

/// Parámetros de escena que el juego envía al renderer.
#[derive(Clone)]
pub struct SceneParams {
    pub camera: Camera,
    /// 0 = medianoche, 0.5 = mediodía.
    pub time_of_day: f32,
    pub underwater: bool,
    pub weather: f32,
    pub exposure: f32,
    pub bloom: f32,
    pub sharpen: f32,
    pub taa: bool,
    pub render_scale: f32,
    pub fog_density: f32,
}

impl Default for SceneParams {
    fn default() -> Self {
        Self {
            camera: Camera::default(),
            time_of_day: 0.35,
            underwater: false,
            weather: 0.0,
            exposure: 1.0,
            bloom: 0.06,
            sharpen: 0.25,
            taa: true,
            render_scale: 1.0,
            fog_density: 1.0,
        }
    }
}

struct GpuChunk {
    vbuf: wgpu::Buffer,
    ibuf: wgpu::Buffer,
    index_count: u32,
    origin: Vec3,
}

/// Resultado de intentar renderizar.
pub enum FrameResult {
    Presented,
    Skip,
}

pub struct Renderer {
    pub ctx: GpuContext,
    pub tier: QualityTier,
    // ── pipelines ──
    terrain_pipe: wgpu::RenderPipeline,
    water_pipe: wgpu::RenderPipeline,
    sky_pipe: wgpu::RenderPipeline,
    shadow_pipe: wgpu::RenderPipeline,
    bright_pipe: wgpu::RenderPipeline,
    blur_h_pipe: wgpu::RenderPipeline,
    blur_v_pipe: wgpu::RenderPipeline,
    tonemap_pipe: wgpu::RenderPipeline,
    upscale_pipe: wgpu::RenderPipeline,
    entity_pipe: wgpu::RenderPipeline,
    entity_buf: wgpu::Buffer,
    entity_cube: wgpu::Buffer,
    entity_capacity: usize,
    entity_count: u32,
    // ── layouts ──
    globals_bgl: wgpu::BindGroupLayout,
    terrain_bgl: wgpu::BindGroupLayout,
    post_bgl: wgpu::BindGroupLayout,
    // ── bind groups ──
    globals_bg: wgpu::BindGroup,
    terrain_bg: wgpu::BindGroup,
    bright_bg: [wgpu::BindGroup; 2],
    blur_bg: [wgpu::BindGroup; 2],
    tonemap_bg: [wgpu::BindGroup; 2],
    upscale_bg: [wgpu::BindGroup; 2],
    chunk_bg: wgpu::BindGroup,
    shadow_chunk_bg: wgpu::BindGroup,
    // ── buffers ──
    globals_buf: wgpu::Buffer,
    post_buf: wgpu::Buffer,
    chunk_ubo: wgpu::Buffer,
    shadow_ubo: wgpu::Buffer,
    // ── texturas ──
    hdr_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
    ldr_views: [wgpu::TextureView; 2],
    bloom_views: [wgpu::TextureView; 2],
    shadow_view: wgpu::TextureView,
    shadow_layer_views: Vec<wgpu::TextureView>,
    // ── estado ──
    pub ui: ui_batch::UiBatch,
    offscreen_tex: wgpu::Texture,
    offscreen_view: wgpu::TextureView,
    chunks: HashMap<IVec3, GpuChunk>,
    frame: u64,
    cur_ldr: usize,
    ldr_valid: [bool; 2],
    prev_view_proj: Mat4,
    size: (u32, u32),
    render_size: (u32, u32),
    pub stats: RenderStats,
}

#[derive(Debug, Default, Clone)]
pub struct RenderStats {
    pub chunks_drawn: usize,
    pub chunks_uploaded: usize,
    pub draw_calls: usize,
    pub quads_ui: usize,
    pub texts_ui: usize,
}

impl Renderer {
    pub fn new(
        window: Option<std::sync::Arc<winit::window::Window>>,
        size: (u32, u32),
        tier: QualityTier,
    ) -> Result<Self> {
        let ctx = pollster::block_on(GpuContext::new(window, size, true))?;
        let device = &ctx.device;

        // ── texturas ──
        let (_hdr_tex, hdr_view) = create_tex(
            device,
            size,
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            Some("hdr"),
        );
        let (depth_tex, depth_view) = create_tex(
            device,
            size,
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            Some("depth"),
        );
        let _ = depth_tex;
        let ldr_a = create_tex(
            device,
            size,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            Some("ldr-a"),
        );
        let ldr_b = create_tex(
            device,
            size,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            Some("ldr-b"),
        );
        let bloom_size = ((size.0 / BLOOM_DIV).max(1), (size.1 / BLOOM_DIV).max(1));
        let bloom_a = create_tex(
            device,
            bloom_size,
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            Some("bloom-a"),
        );
        let bloom_b = create_tex(
            device,
            bloom_size,
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            Some("bloom-b"),
        );
        // Sombras: array de CASCADES capas.
        let shadow_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow-array"),
            size: wgpu::Extent3d {
                width: SHADOW_RES,
                height: SHADOW_RES,
                depth_or_array_layers: CASCADES as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow_tex.create_view(&wgpu::TextureViewDescriptor {
            label: Some("shadow-array-view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let shadow_layer_views: Vec<wgpu::TextureView> = (0..CASCADES)
            .map(|i| {
                shadow_tex.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("shadow-layer"),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: i as u32,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();

        // ── samplers ──
        let lin_smp = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let shadow_smp = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow-compare"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // ── uniforms ──
        let globals_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let post_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("post-params"),
            size: std::mem::size_of::<PostParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ubo_alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
        let chunk_stride = align_up(std::mem::size_of::<ChunkInfo>() as u64, ubo_alignment);
        let chunk_ubo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("chunk-offsets"),
            size: chunk_stride * MAX_CHUNKS as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shadow_ubo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow-offsets"),
            size: chunk_stride * MAX_CHUNKS as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ── layouts ──
        let globals_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let terrain_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let post_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("post-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let chunk_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("chunk-bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<ChunkInfo>() as u64),
                },
                count: None,
            }],
        });

        // ── bind groups ──
        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals-bg"),
            layout: &globals_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buf.as_entire_binding(),
            }],
        });
        let terrain_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain-bg"),
            layout: &terrain_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: globals_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&shadow_smp),
                },
            ],
        });
        let chunk_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("chunk-bg"),
            layout: &chunk_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &chunk_ubo,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<ChunkInfo>() as u64),
                }),
            }],
        });
        let shadow_chunk_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow-chunk-bg"),
            layout: &chunk_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &shadow_ubo,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<ChunkInfo>() as u64),
                }),
            }],
        });
        let post_bg = |label, src: &wgpu::TextureView, bloom: &wgpu::TextureView, depth: &wgpu::TextureView, hist: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &post_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(src),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(bloom),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(depth),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(hist),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&lin_smp),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: post_buf.as_entire_binding(),
                    },
                ],
            })
        };
        let bright_bg = [
            post_bg("bright-0", &hdr_view, &hdr_view, &depth_view, &hdr_view),
            post_bg("bright-1", &hdr_view, &hdr_view, &depth_view, &hdr_view),
        ];
        let blur_bg = [
            post_bg("blur-0", &bloom_a.1, &bloom_a.1, &depth_view, &bloom_a.1),
            post_bg("blur-1", &bloom_b.1, &bloom_b.1, &depth_view, &bloom_b.1),
        ];
        let tonemap_bg = [
            post_bg("tonemap-0", &hdr_view, &bloom_a.1, &depth_view, &ldr_b.1),
            post_bg("tonemap-1", &hdr_view, &bloom_a.1, &depth_view, &ldr_a.1),
        ];
        let upscale_bg = [
            post_bg("upscale-0", &ldr_a.1, &ldr_a.1, &depth_view, &ldr_a.1),
            post_bg("upscale-1", &ldr_b.1, &ldr_b.1, &depth_view, &ldr_b.1),
        ];

        // ── shaders + pipelines ──
        let terrain_shader = shader(device, "terrain", include_str!("../../../assets/shaders/terrain.wgsl"));
        let water_shader = shader(device, "water", include_str!("../../../assets/shaders/water.wgsl"));
        let sky_shader = shader(device, "sky", include_str!("../../../assets/shaders/sky.wgsl"));
        let shadow_shader = shader(device, "shadow", include_str!("../../../assets/shaders/shadow.wgsl"));
        let post_shader = shader(device, "post", include_str!("../../../assets/shaders/post.wgsl"));

        let terrain_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terrain-layout"),
            bind_group_layouts: &[Some(&terrain_bgl), Some(&chunk_bgl)],
            immediate_size: 0,
        });
        let globals_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("globals-layout"),
            bind_group_layouts: &[Some(&globals_bgl)],
            immediate_size: 0,
        });
        let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow-layout"),
            bind_group_layouts: &[Some(&globals_bgl), Some(&chunk_bgl)],
            immediate_size: 0,
        });
        let post_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("post-layout"),
            bind_group_layouts: &[Some(&globals_bgl), Some(&post_bgl)],
            immediate_size: 0,
        });

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32x2,
                offset: 0,
                shader_location: 0,
            }],
        };
        let color_target = |format| {
            Some(wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })
        };
        let terrain_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terrain"),
            layout: Some(&terrain_layout),
            vertex: wgpu::VertexState {
                module: &terrain_shader,
                entry_point: Some("vs_main"),
                buffers: std::slice::from_ref(&vertex_layout),
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &terrain_shader,
                entry_point: Some("fs_main"),
                targets: &[color_target(wgpu::TextureFormat::Rgba16Float)],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let water_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("water"),
            layout: Some(&terrain_layout),
            vertex: wgpu::VertexState {
                module: &water_shader,
                entry_point: Some("vs_main"),
                buffers: std::slice::from_ref(&vertex_layout),
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &water_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let sky_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&globals_layout),
            vertex: wgpu::VertexState {
                module: &sky_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &sky_shader,
                entry_point: Some("fs_main"),
                targets: &[color_target(wgpu::TextureFormat::Rgba16Float)],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let shadow_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow"),
            layout: Some(&shadow_layout),
            vertex: wgpu::VertexState {
                module: &shadow_shader,
                entry_point: Some("vs_main"),
                buffers: std::slice::from_ref(&vertex_layout),
                compilation_options: Default::default(),
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        let make_post_pipe = |name: &str, entry: &str, targets: &[Option<wgpu::ColorTargetState>]| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(name),
                layout: Some(&post_layout),
                vertex: wgpu::VertexState {
                    module: &post_shader,
                    entry_point: Some("vs_full"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &post_shader,
                    entry_point: Some(entry),
                    targets,
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let bloom_target = Some(wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Rgba16Float,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        });
        let bright_pipe = make_post_pipe("bright", "fs_bright", std::slice::from_ref(&bloom_target));
        let blur_h_pipe = make_post_pipe("blur-h", "fs_blur", std::slice::from_ref(&bloom_target));
        let blur_v_pipe = make_post_pipe("blur-v", "fs_blur_v", std::slice::from_ref(&bloom_target));
        let ldr_target = Some(wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        });
        let tonemap_pipe = make_post_pipe("tonemap", "fs_tonemap", std::slice::from_ref(&ldr_target));
        let upscale_pipe = make_post_pipe(
            "upscale",
            "fs_upscale",
            &[Some(wgpu::ColorTargetState {
                format: ctx.format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        );

        // ── Entidades (cajas instanciadas) ──
        let entity_shader = shader(device, "entity", include_str!("../../../assets/shaders/entity.wgsl"));
        let entity_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("entity"),
            layout: Some(&globals_layout),
            vertex: wgpu::VertexState {
                module: &entity_shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: 12,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        }],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<EntityInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 0,
                                shader_location: 1,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x3,
                                offset: 16,
                                shader_location: 2,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 32,
                                shader_location: 3,
                            },
                        ],
                    },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &entity_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                front_face: wgpu::FrontFace::Ccw,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let entity_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("entities"),
            size: (4096 * std::mem::size_of::<EntityInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Cubo unitario (36 vértices, 12 triángulos).
        const CUBE: [[f32; 3]; 36] = [
            // +X
            [0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [0.5, 0.5, 0.5],
            [0.5, -0.5, -0.5], [0.5, 0.5, 0.5], [0.5, -0.5, 0.5],
            // -X
            [-0.5, -0.5, 0.5], [-0.5, 0.5, 0.5], [-0.5, 0.5, -0.5],
            [-0.5, -0.5, 0.5], [-0.5, 0.5, -0.5], [-0.5, -0.5, -0.5],
            // +Y
            [-0.5, 0.5, -0.5], [-0.5, 0.5, 0.5], [0.5, 0.5, 0.5],
            [-0.5, 0.5, -0.5], [0.5, 0.5, 0.5], [0.5, 0.5, -0.5],
            // -Y
            [-0.5, -0.5, 0.5], [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5],
            [-0.5, -0.5, 0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5],
            // +Z
            [-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.5],
            [-0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5],
            // -Z
            [0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [-0.5, 0.5, -0.5],
            [0.5, -0.5, -0.5], [-0.5, 0.5, -0.5], [0.5, 0.5, -0.5],
        ];
        let entity_cube = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("entity-cube"),
            size: std::mem::size_of_val(&CUBE) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.queue.write_buffer(&entity_cube, 0, bytemuck::cast_slice(&CUBE));
        let _ = &entity_shader;

        let ui = ui_batch::UiBatch::new(device, &ctx.queue, ctx.format);
        // Target offscreen para modo headless (tests/CI).
        let (offscreen_tex, offscreen_view) = create_tex(
            device,
            size,
            ctx.format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            Some("offscreen"),
        );

        let _ = (water_shader, ldr_a.0, ldr_b.0, bloom_a.0, bloom_b.0, lin_smp);
        Ok(Self {
            ctx,
            tier,
            terrain_pipe,
            water_pipe,
            sky_pipe,
            shadow_pipe,
            bright_pipe,
            blur_h_pipe,
            blur_v_pipe,
            tonemap_pipe,
            upscale_pipe,
            entity_pipe,
            entity_buf,
            entity_cube,
            entity_capacity: 4096,
            entity_count: 0,
            globals_bgl,
            terrain_bgl,
            post_bgl,
            globals_bg,
            terrain_bg,
            bright_bg,
            blur_bg,
            tonemap_bg,
            upscale_bg,
            chunk_bg,
            shadow_chunk_bg,
            globals_buf,
            post_buf,
            chunk_ubo,
            shadow_ubo,
            hdr_view,
            depth_view,
            ldr_views: [ldr_a.1, ldr_b.1],
            bloom_views: [bloom_a.1, bloom_b.1],
            shadow_view,
            shadow_layer_views,
            ui,
            offscreen_tex,
            offscreen_view,
            chunks: HashMap::new(),
            frame: 0,
            cur_ldr: 0,
            ldr_valid: [false, false],
            prev_view_proj: Mat4::IDENTITY,
            size,
            render_size: size,
            stats: RenderStats::default(),
        })
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.size = (w, h);
        self.ctx.resize(w, h);
        self.recreate_targets();
    }

    fn recreate_targets(&mut self) {
        let device = &self.ctx.device;
        let scale = self.tier.render_scale().clamp(0.4, 1.0);
        let rw = ((self.size.0 as f32 * scale) as u32).max(1);
        let rh = ((self.size.1 as f32 * scale) as u32).max(1);
        self.render_size = (rw, rh);
        let hdr = create_tex(
            device,
            (rw, rh),
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            Some("hdr"),
        );
        let depth = create_tex(
            device,
            (rw, rh),
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            Some("depth"),
        );
        let ldr_a = create_tex(device, (rw, rh), wgpu::TextureFormat::Rgba8UnormSrgb, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING, Some("ldr-a"));
        let ldr_b = create_tex(device, (rw, rh), wgpu::TextureFormat::Rgba8UnormSrgb, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING, Some("ldr-b"));
        let bs = ((rw / BLOOM_DIV).max(1), (rh / BLOOM_DIV).max(1));
        let bloom_a = create_tex(device, bs, wgpu::TextureFormat::Rgba16Float, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING, Some("bloom-a"));
        let bloom_b = create_tex(device, bs, wgpu::TextureFormat::Rgba16Float, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING, Some("bloom-b"));
        self.hdr_view = hdr.1;
        self.depth_view = depth.1;
        self.ldr_views = [ldr_a.1, ldr_b.1];
        self.bloom_views = [bloom_a.1, bloom_b.1];
        self.ldr_valid = [false, false];
        // Recrea bind groups que apuntan a texturas de tamaño variable.
        let lin_smp = self.ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shadow_smp = self.ctx.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow-compare"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let mk = |label: &str, src: &wgpu::TextureView, bloom: &wgpu::TextureView, hist: &wgpu::TextureView| {
            self.ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &self.post_bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(src) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(bloom) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.depth_view) },
                    wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(hist) },
                    wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&lin_smp) },
                    wgpu::BindGroupEntry { binding: 5, resource: self.post_buf.as_entire_binding() },
                ],
            })
        };
        self.bright_bg = [
            mk("bright-0", &self.hdr_view, &self.hdr_view, &self.hdr_view),
            mk("bright-1", &self.hdr_view, &self.hdr_view, &self.hdr_view),
        ];
        self.blur_bg = [
            mk("blur-0", &self.bloom_views[0], &self.bloom_views[0], &self.bloom_views[0]),
            mk("blur-1", &self.bloom_views[1], &self.bloom_views[1], &self.bloom_views[1]),
        ];
        self.tonemap_bg = [
            mk("tonemap-0", &self.hdr_view, &self.bloom_views[0], &self.ldr_views[1]),
            mk("tonemap-1", &self.hdr_view, &self.bloom_views[0], &self.ldr_views[0]),
        ];
        self.upscale_bg = [
            mk("upscale-0", &self.ldr_views[0], &self.ldr_views[0], &self.ldr_views[0]),
            mk("upscale-1", &self.ldr_views[1], &self.ldr_views[1], &self.ldr_views[1]),
        ];
        self.globals_bg = self.ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals-bg"),
            layout: &self.globals_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.globals_buf.as_entire_binding(),
            }],
        });
        self.terrain_bg = self.ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("terrain-bg"),
            layout: &self.terrain_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.globals_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.shadow_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&shadow_smp) },
            ],
        });
        let _ = &self.post_bgl;
    }

    /// Sube las instancias de entidades del frame (una sola escritura).
    pub fn set_entities(&mut self, entities: &[EntityInstance]) {
        let n = entities.len().min(self.entity_capacity);
        self.entity_count = n as u32;
        if n > 0 {
            self.ctx
                .queue
                .write_buffer(&self.entity_buf, 0, bytemuck::cast_slice(&entities[..n]));
        }
    }

    /// Captura el buffer offscreen a un PPM (solo modo headless, para tests).
    pub fn capture_ppm(&self, path: &str) -> Result<()> {
        let (w, h) = self.size;
        let bpp = 8u32; // Rgba16Float
        let unpadded = w * bpp;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded = unpadded.div_ceil(align) * align;
        let buf = self.ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture"),
            size: (padded as u64) * (h as u64),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("capture") });
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.offscreen_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.ctx.queue.submit(Some(enc.finish()));
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.ctx.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        let data = slice.get_mapped_range();
        let mut out = Vec::with_capacity((w * h * 3) as usize);
        for y in 0..h {
            for x in 0..w {
                let off = (y * padded + x * bpp) as usize;
                let r = f16_to_f32(u16::from_le_bytes([data[off], data[off + 1]]));
                let g = f16_to_f32(u16::from_le_bytes([data[off + 2], data[off + 3]]));
                let b = f16_to_f32(u16::from_le_bytes([data[off + 4], data[off + 5]]));
                out.push((srgb(r) * 255.0) as u8);
                out.push((srgb(g) * 255.0) as u8);
                out.push((srgb(b) * 255.0) as u8);
            }
        }
        drop(data);
        buf.unmap();
        let mut file = std::fs::File::create(path)?;
        use std::io::Write;
        write!(file, "P6\n{} {}\n255\n", w, h)?;
        file.write_all(&out)?;
        Ok(())
    }

    // ── Chunks ──────────────────────────────────────────────────────

    pub fn upload_chunk(&mut self, key: IVec3, mesh: &rivaren_meshing::MeshData) {
        if mesh.vertices.is_empty() || mesh.indices.is_empty() {
            self.chunks.remove(&key);
            return;
        }
        let vbytes: &[u8] = bytemuck::cast_slice(&mesh.vertices);
        let ibytes: &[u8] = bytemuck::cast_slice(&mesh.indices);
        let device = &self.ctx.device;
        let vbuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("chunk-v"),
            size: vbytes.len() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ibuf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("chunk-i"),
            size: ibytes.len() as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.ctx.queue.write_buffer(&vbuf, 0, vbytes);
        self.ctx.queue.write_buffer(&ibuf, 0, ibytes);
        self.chunks.insert(
            key,
            GpuChunk {
                vbuf,
                ibuf,
                index_count: mesh.indices.len() as u32,
                origin: Vec3::new((key.x * 32) as f32, (key.y * 32) as f32, (key.z * 32) as f32),
            },
        );
    }

    pub fn remove_chunk(&mut self, key: IVec3) {
        self.chunks.remove(&key);
    }

    pub fn has_chunk(&self, key: IVec3) -> bool {
        self.chunks.contains_key(&key)
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    // ── Frame ───────────────────────────────────────────────────────

    pub fn render(&mut self, scene: &SceneParams, draw: &rivaren_core::DrawList) -> Result<FrameResult> {
        self.stats = RenderStats::default();
        let frame = match &self.ctx.surface {
            Some(s) => match s.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(f)
                | wgpu::CurrentSurfaceTexture::Suboptimal(f) => Some(f),
                wgpu::CurrentSurfaceTexture::Outdated => {
                    if std::env::var("RIVAREN_DEBUG").is_ok() {
                        tracing::info!("surface Outdated -> reconfigure");
                    }
                    if let Some(cfg) = self.ctx.surface_config.clone() {
                        s.configure(&self.ctx.device, &cfg);
                    }
                    return Ok(FrameResult::Skip);
                }
                wgpu::CurrentSurfaceTexture::Timeout => {
                    if std::env::var("RIVAREN_DEBUG").is_ok() && self.frame.is_multiple_of(120) {
                        tracing::info!("surface Timeout");
                    }
                    return Ok(FrameResult::Skip);
                }
                wgpu::CurrentSurfaceTexture::Occluded => {
                    if std::env::var("RIVAREN_DEBUG").is_ok() && self.frame.is_multiple_of(120) {
                        tracing::info!("surface Occluded");
                    }
                    return Ok(FrameResult::Skip);
                }
                wgpu::CurrentSurfaceTexture::Lost => {
                    tracing::warn!("surface Lost");
                    return Ok(FrameResult::Skip);
                }
                other => {
                    tracing::warn!("surface acquire: {other:?}");
                    return Ok(FrameResult::Skip);
                }
            },
            None => None,
        };

        let cam = &scene.camera;
        let view_proj = cam.view_proj();
        let sun_dir = sun_direction(scene.time_of_day);
        let sun_intensity = (sun_dir.y * 1.6).clamp(0.08, 1.0);
        let day = (sun_dir.y * 2.0).clamp(0.0, 1.0);

        // Cielo por hora.
        let zenith = mix3([0.05, 0.08, 0.18], [0.32, 0.52, 0.92], day);
        let horizon = mix3([0.06, 0.07, 0.14], [0.62, 0.72, 0.95], day);
        let cascades = cascade_matrices(cam, sun_dir, self.tier.shadows());
        let globals = Globals {
            view_proj: view_proj.to_cols_array_2d(),
            prev_view_proj: self.prev_view_proj.to_cols_array_2d(),
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            cam_pos: [cam.pos.x, cam.pos.y, cam.pos.z, self.frame as f32 * 0.016],
            sun_dir: [sun_dir.x, sun_dir.y, sun_dir.z, sun_intensity],
            sky_color: [zenith[0], zenith[1], zenith[2], scene.fog_density],
            horizon_color: [horizon[0], horizon[1], horizon[2], 1.0],
            cascade_mats: cascades.map(|m| m.to_cols_array_2d()),
            cascade_splits: [
                cascade_splits()[0],
                cascade_splits()[1],
                cascade_splits()[2],
                self.tier.shadows() as f32,
            ],
            misc: [
                scene.time_of_day,
                scene.exposure,
                if scene.underwater { 1.0 } else { 0.0 },
                scene.weather,
            ],
        };
        self.ctx.queue.write_buffer(&self.globals_buf, 0, bytemuck::bytes_of(&globals));
        let post = PostParams {
            texel: [
                1.0 / self.render_size.0 as f32,
                1.0 / self.render_size.1 as f32,
            ],
            bloom_strength: scene.bloom,
            exposure: scene.exposure,
            taa_blend: if scene.taa && self.ldr_valid[1 - self.cur_ldr] {
                0.88
            } else {
                0.0
            },
            frame: self.frame as f32,
            underwater: if scene.underwater { 1.0 } else { 0.0 },
            sharpen: scene.sharpen,
            _pad: 0.0,
            _pad2: 0.0,
        };
        self.ctx.queue.write_buffer(&self.post_buf, 0, bytemuck::bytes_of(&post));

        // ── prepara UBOs de chunks (terrain + shadow) ──
        let planes = cam.frustum_planes();
        let stride = align_up(
            std::mem::size_of::<ChunkInfo>() as u64,
            self.ctx.device.limits().min_uniform_buffer_offset_alignment as u64,
        );
        let mut terrain_slots: Vec<(IVec3, u32)> = Vec::new();
        let mut terrain_data: Vec<u8> = Vec::with_capacity(self.chunks.len() * stride as usize);
        let mut shadow_slots: Vec<(IVec3, u32, u32)> = Vec::new();
        let mut shadow_data: Vec<u8> = Vec::new();
        let cam_chunk = IVec3::new(
            (cam.pos.x / 32.0).floor() as i32,
            (cam.pos.y / 32.0).floor() as i32,
            (cam.pos.z / 32.0).floor() as i32,
        );
        let max_dist = cam.far;
        for (key, chunk) in &self.chunks {
            let min = chunk.origin;
            let max = min + Vec3::splat(32.0);
            let center = min + Vec3::splat(16.0);
            let dist = center.distance(cam.pos);
            let visible = aabb_in_frustum(&planes, min, max) && dist < max_dist;
            if visible {
                let idx = terrain_slots.len() as u32;
                if idx as usize >= MAX_CHUNKS {
                    break;
                }
                terrain_slots.push((*key, idx));
                push_chunk_info(&mut terrain_data, &ChunkInfo { offset: [min.x, min.y, min.z, 0.0] }, stride);
            }
            // Sombras: solo alrededor del jugador y dentro del alcance de la última cascada.
            if dist < cascade_splits()[2] + 48.0 && visible {
                for c in 0..self.tier.shadows() {
                    let s = shadow_slots.len() as u32;
                    if s as usize >= MAX_CHUNKS {
                        break;
                    }
                    shadow_slots.push((*key, c as u32, s));
                    push_chunk_info(
                        &mut shadow_data,
                        &ChunkInfo {
                            offset: [min.x, min.y, min.z, c as f32],
                        },
                        stride,
                    );
                }
            }
            let _ = cam_chunk;
        }
        if !terrain_data.is_empty() {
            self.ctx.queue.write_buffer(&self.chunk_ubo, 0, &terrain_data);
        }
        if !shadow_data.is_empty() {
            self.ctx.queue.write_buffer(&self.shadow_ubo, 0, &shadow_data);
        }

        // ── encoding ──
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

        // 1. Sombras CSM.
        if !shadow_slots.is_empty() {
            for c in 0..self.tier.shadows() {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("shadow-pass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.shadow_layer_views[c],
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&self.shadow_pipe);
                pass.set_bind_group(0, &self.globals_bg, &[]);
                pass.set_bind_group(1, &self.shadow_chunk_bg, &[0]);
                let mut last_buf: Option<*const wgpu::Buffer> = None;
                for (key, cc, slot) in &shadow_slots {
                    if *cc != c as u32 {
                        continue;
                    }
                    if let Some(chunk) = self.chunks.get(key) {
                        let p = chunk as *const GpuChunk;
                        if last_buf != Some(&chunk.vbuf as *const _) {
                            pass.set_vertex_buffer(0, chunk.vbuf.slice(..));
                            last_buf = Some(&chunk.vbuf as *const _);
                        }
                        pass.set_index_buffer(chunk.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                        let offset = (*slot as u64) * stride;
                        pass.set_bind_group(1, &self.shadow_chunk_bg, &[offset as u32]);
                        pass.draw_indexed(0..chunk.index_count, 0, 0..1);
                        self.stats.draw_calls += 1;
                        let _ = p;
                    }
                }
            }
        }

        // 2. Cielo + terreno + agua en un solo render pass al HDR.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            // Cielo.
            pass.set_pipeline(&self.sky_pipe);
            pass.set_bind_group(0, &self.globals_bg, &[]);
            pass.draw(0..3, 0..1);
            self.stats.draw_calls += 1;
            // Terreno.
            pass.set_pipeline(&self.terrain_pipe);
            pass.set_bind_group(0, &self.terrain_bg, &[]);
            let mut last_vbuf: Option<*const wgpu::Buffer> = None;
            for (key, slot) in &terrain_slots {
                if let Some(chunk) = self.chunks.get(key) {
                    if last_vbuf != Some(&chunk.vbuf as *const _) {
                        pass.set_vertex_buffer(0, chunk.vbuf.slice(..));
                        last_vbuf = Some(&chunk.vbuf as *const _);
                    }
                    pass.set_index_buffer(chunk.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    let offset = (*slot as u64) * stride;
                    pass.set_bind_group(1, &self.chunk_bg, &[offset as u32]);
                    pass.draw_indexed(0..chunk.index_count, 0, 0..1);
                    self.stats.chunks_drawn += 1;
                    self.stats.draw_calls += 1;
                }
            }
            // Entidades (cajas animadas).
            if self.entity_count > 0 {
                pass.set_pipeline(&self.entity_pipe);
                pass.set_bind_group(0, &self.globals_bg, &[]);
                pass.set_vertex_buffer(0, self.entity_cube.slice(..));
                pass.set_vertex_buffer(1, self.entity_buf.slice(..));
                pass.draw(0..36, 0..self.entity_count);
                self.stats.draw_calls += 1;
            }
            // Agua (reusa los mismos meshes, clip de no-agua en el shader).
            pass.set_pipeline(&self.water_pipe);
            pass.set_bind_group(0, &self.terrain_bg, &[]);
            let mut last_vbuf: Option<*const wgpu::Buffer> = None;
            for (key, slot) in &terrain_slots {
                if let Some(chunk) = self.chunks.get(key) {
                    if last_vbuf != Some(&chunk.vbuf as *const _) {
                        pass.set_vertex_buffer(0, chunk.vbuf.slice(..));
                        last_vbuf = Some(&chunk.vbuf as *const _);
                    }
                    pass.set_index_buffer(chunk.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    let offset = (*slot as u64) * stride;
                    pass.set_bind_group(1, &self.chunk_bg, &[offset as u32]);
                    pass.draw_indexed(0..chunk.index_count, 0, 0..1);
                    self.stats.draw_calls += 1;
                }
            }
        }

        // 3. Bloom: bright → blur H → blur V.
        let bloom_target_size = (
            (self.render_size.0 / BLOOM_DIV).max(1),
            (self.render_size.1 / BLOOM_DIV).max(1),
        );
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bloom-bright"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_views[0],
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.bright_pipe);
            pass.set_bind_group(0, &self.globals_bg, &[]);
            pass.set_bind_group(1, &self.bright_bg[0], &[]);
            pass.set_viewport(
                0.0,
                0.0,
                bloom_target_size.0 as f32,
                bloom_target_size.1 as f32,
                0.0,
                1.0,
            );
            pass.draw(0..3, 0..1);
            self.stats.draw_calls += 1;
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blur-h"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_views[1],
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.blur_h_pipe);
            pass.set_bind_group(0, &self.globals_bg, &[]);
            pass.set_bind_group(1, &self.blur_bg[0], &[]);
            pass.set_viewport(0.0, 0.0, bloom_target_size.0 as f32, bloom_target_size.1 as f32, 0.0, 1.0);
            pass.draw(0..3, 0..1);
            self.stats.draw_calls += 1;
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blur-v"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_views[0],
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.blur_v_pipe);
            pass.set_bind_group(0, &self.globals_bg, &[]);
            pass.set_bind_group(1, &self.blur_bg[1], &[]);
            pass.set_viewport(0.0, 0.0, bloom_target_size.0 as f32, bloom_target_size.1 as f32, 0.0, 1.0);
            pass.draw(0..3, 0..1);
            self.stats.draw_calls += 1;
        }

        // 4. Tonemap + TAA → LDR render-res.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("tonemap"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.ldr_views[self.cur_ldr],
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.tonemap_pipe);
            pass.set_bind_group(0, &self.globals_bg, &[]);
            pass.set_bind_group(1, &self.tonemap_bg[self.cur_ldr], &[]);
            pass.set_viewport(
                0.0,
                0.0,
                self.render_size.0 as f32,
                self.render_size.1 as f32,
                0.0,
                1.0,
            );
            pass.draw(0..3, 0..1);
            self.stats.draw_calls += 1;
        }

        // 5. Upscale + sharpen → surface (o textura offscreen en headless).
        let target_view = if let Some(f) = &frame {
            f.texture.create_view(&wgpu::TextureViewDescriptor::default())
        } else {
            self.offscreen_view.clone()
        };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("upscale"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.upscale_pipe);
            pass.set_bind_group(0, &self.globals_bg, &[]);
            pass.set_bind_group(1, &self.upscale_bg[self.cur_ldr], &[]);
            pass.set_viewport(0.0, 0.0, self.size.0 as f32, self.size.1 as f32, 0.0, 1.0);
            pass.draw(0..3, 0..1);
            self.stats.draw_calls += 1;
        }

        // 6. UI: quads + texto.
        self.ui.render(
            &self.ctx.device,
            &self.ctx.queue,
            &mut encoder,
            &target_view,
            draw,
            self.size,
        )?;
        self.stats.quads_ui = draw.quads.len();
        self.stats.texts_ui = draw.texts.len();

        self.ctx.queue.submit(Some(encoder.finish()));
        if let Some(f) = frame {
            f.present();
        }
        self.prev_view_proj = view_proj;
        self.frame += 1;
        self.ldr_valid[self.cur_ldr] = true;
        self.cur_ldr ^= 1;
        Ok(FrameResult::Presented)
    }
}

// ── helpers ──────────────────────────────────────────────────────────

fn shader(device: &wgpu::Device, label: &str, src: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(src.into()),
    })
}

fn create_tex(
    device: &wgpu::Device,
    size: (u32, u32),
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    label: Option<&str>,
) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label,
        size: wgpu::Extent3d {
            width: size.0.max(1),
            height: size.1.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    });
    let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    (tex, view)
}

fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1F) as u32;
    let frac = (h & 0x3FF) as u32;
    let bits = if exp == 0 {
        if frac == 0 {
            sign << 31
        } else {
            // subnormal
            let mut e = 127 - 15 + 1;
            let mut f = frac;
            while f & 0x400 == 0 {
                f <<= 1;
                e -= 1;
            }
            (sign << 31) | ((e as u32) << 23) | ((f & 0x3FF) << 13)
        }
    } else if exp == 31 {
        (sign << 31) | 0x7F80_0000 | (frac << 13)
    } else {
        (sign << 31) | ((exp + 127 - 15) << 23) | (frac << 13)
    };
    f32::from_bits(bits)
}

fn srgb(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

fn align_up(v: u64, align: u64) -> u64 {
    v.div_ceil(align) * align
}

fn push_chunk_info(data: &mut Vec<u8>, info: &ChunkInfo, stride: u64) {
    let start = data.len();
    let bytes = bytemuck::bytes_of(info);
    data.extend_from_slice(bytes);
    data.resize(start + stride as usize, 0);
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

pub fn sun_direction(time_of_day: f32) -> Vec3 {
    let a = (time_of_day - 0.25) * std::f32::consts::TAU;
    Vec3::new(a.cos() * 0.4, a.sin(), a.cos() * 0.8).normalize()
}

pub fn cascade_splits() -> [f32; 3] {
    [24.0, 72.0, 180.0]
}

/// Calcula las matrices view-proj de luz para CSM.
fn cascade_matrices(cam: &Camera, sun_dir: Vec3, count: usize) -> [Mat4; CASCADES] {
    let splits = cascade_splits();
    let mut out = [Mat4::IDENTITY; CASCADES];
    let inv = cam.view_proj().inverse();
    let mut near = cam.near;
    for i in 0..count.min(CASCADES) {
        let far = splits[i];
        // 8 esquinas del frustum slice en mundo.
        let mut corners: [Vec3; 8] = [Vec3::ZERO; 8];
        let mut n = 0;
        for z in [near, far] {
            for y in [-1.0f32, 1.0] {
                for x in [-1.0f32, 1.0] {
                    let p = inv * Vec4::new(x, y, z, 1.0);
                    corners[n] = p.truncate() / p.w;
                    n += 1;
                }
            }
        }
        let center = corners.iter().copied().sum::<Vec3>() / 8.0;
        let radius = corners
            .iter()
            .map(|c| c.distance(center))
            .fold(0.0f32, f32::max)
            .max(4.0);
        // Redondea el radio a texel del shadow map.
        let texels = SHADOW_RES as f32;
        let world_per_texel = (radius * 2.0) / texels;
        let radius = (radius / world_per_texel).ceil() * world_per_texel;
        let eye = center - sun_dir * (radius * 2.0);
        let up = if sun_dir.y.abs() > 0.95 {
            Vec3::Z
        } else {
            Vec3::Y
        };
        let view = Mat4::look_at_rh(eye, center, up);
        let proj = Mat4::orthographic_rh(-radius, radius, -radius, radius, 0.1, radius * 4.5);
        out[i] = proj * view;
        near = far;
    }
    out
}
