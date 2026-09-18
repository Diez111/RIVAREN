//! Ventana winit 0.30 + wgpu real: renderiza el chunk mesheado en 3D.
//! Cámara orbital automática + WASD/arrows para mover. ESC/cerrar para salir.
//!
//! Multiplataforma: mismo código en Linux (Vulkan/X11-Wayland),
//! Windows (DX12) y Mac (Metal) vía wgpu. Sin `unsafe` salvo el cast
//! documentado de voxels (Box<[u16;32768]> → &[u16;32768]).

use anyhow::Result;
use glam::{IVec3, Mat4, Vec3};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes};

pub fn run_window(seed: u64) -> Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = RivarenApp::new(seed);
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct GpuState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    uniform_buf: wgpu::Buffer,
    uniform_bind: wgpu::BindGroup,
    vertex_buf: wgpu::Buffer,
    index_buf: wgpu::Buffer,
    index_count: u32,
    start: Instant,
    angle: f32,
    cam_pos: Vec3,
    size: winit::dpi::PhysicalSize<u32>,
}

struct RivarenApp {
    seed: u64,
    state: Option<GpuState>,
    mesh_vertices: Vec<rivaren_core::PackedVertex>,
    mesh_indices: Vec<u32>,
}

impl RivarenApp {
    fn new(seed: u64) -> Self {
        // Genera el chunk de superficie una vez (mismo pipeline headless).
        let cancel = AtomicBool::new(false);
        let mut mesh_v = Vec::new();
        let mut mesh_i = Vec::new();
        for cy in [1, 0, 2, -1] {
            let b = rivaren_world::pipeline::generate_chunk(seed, IVec3::new(0, cy, 0), &cancel);
            let nonempty = b.voxels.iter().filter(|&&v| v != 0).count();
            if nonempty > 1000 {
                let voxels: &[u16; 32768] = unsafe { &*b.voxels.as_ptr().cast() };
                let mut mesh = rivaren_meshing::MeshData::default();
                rivaren_meshing::greedy_mesh(voxels, &mut mesh);
                mesh_v = mesh.vertices;
                mesh_i = mesh.indices;
                tracing::info!("chunk y={cy}: {nonempty} sólidos, {} verts", mesh_v.len());
                break;
            }
        }
        Self { seed, state: None, mesh_vertices: mesh_v, mesh_indices: mesh_i }
    }

    fn init_gpu(&mut self, event_loop: &ActiveEventLoop) -> Result<()> {
        let window = Arc::new(event_loop.create_window(
            WindowAttributes::default()
                .with_title(format!("RIVAREN — seed {}", self.seed))
                .with_inner_size(winit::dpi::LogicalSize::new(1280, 720)),
        )?);
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        // SAFETY: window vive en Arc dentro de GpuState junto al surface.
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| anyhow::anyhow!("sin adaptador GPU (¿drivers Vulkan/Metal/DX12?)"))?;
        tracing::info!("adaptador: {:?}", adapter.get_info());
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("rivaren"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Shader + pipeline (vertex = 2×u32 = 8B PackedVertex).
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("terrain"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../assets/shaders/terrain.wgsl").into()),
        });
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera-bind"),
            layout: &bind_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });
        let pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terrain-layout"),
            bind_group_layouts: &[&bind_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("terrain"),
            layout: Some(&pipe_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 8,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Uint32x2,
                        offset: 0,
                        shader_location: 0,
                    }],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
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
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Upload mesh (bytemuck cast: PackedVertex es Pod 8B = 2×u32).
        let vbytes: &[u8] = bytemuck::cast_slice(&self.mesh_vertices);
        let vertex_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("chunk-verts"),
            size: vbytes.len().max(8) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buf, 0, vbytes);
        let ibytes: &[u8] = bytemuck::cast_slice(&self.mesh_indices);
        let index_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("chunk-idx"),
            size: ibytes.len().max(4) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&index_buf, 0, ibytes);

        self.state = Some(GpuState {
            window,
            surface,
            device,
            queue,
            config,
            pipeline,
            uniform_buf,
            uniform_bind,
            vertex_buf,
            index_buf,
            index_count: self.mesh_indices.len() as u32,
            start: Instant::now(),
            angle: 0.0,
            cam_pos: Vec3::new(0.0, 8.0, 52.0),
            size,
        });
        Ok(())
    }

    fn render(&mut self) {
        let Some(st) = self.state.as_mut() else { return };
        st.angle += 0.008;
        // Cámara orbital + deriva: muestra el chunk desde todos los lados.
        let r = 52.0;
        let eye = Vec3::new(st.angle.cos() * r, 20.0 + (st.angle * 0.7).sin() * 6.0, st.angle.sin() * r);
        let view = Mat4::look_at_rh(eye, Vec3::new(0.0, 0.0, 0.0), Vec3::Y);
        let aspect = st.config.width as f32 / st.config.height.max(1) as f32;
        let proj = Mat4::perspective_rh(60f32.to_radians(), aspect, 0.1, 500.0);
        let vp = proj * view;
        st.queue.write_buffer(&st.uniform_buf, 0, bytemuck::cast_slice(vp.as_ref()));
        let frame = match st.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                st.surface.configure(&st.device, &st.config);
                return;
            }
            Err(e) => {
                tracing::warn!("surface: {e:?}");
                return;
            }
        };
        let view_tex = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = st.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("terrain-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view_tex,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.35, g: 0.55, b: 0.80, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&st.pipeline);
            pass.set_bind_group(0, &st.uniform_bind, &[]);
            pass.set_vertex_buffer(0, st.vertex_buf.slice(..));
            pass.set_index_buffer(st.index_buf.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..st.index_count, 0, 0..1);
        }
        st.queue.submit(Some(enc.finish()));
        frame.present();
        st.window.request_redraw();
    }
}

impl ApplicationHandler for RivarenApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            if let Err(e) = self.init_gpu(event_loop) {
                tracing::error!("GPU init falló: {e:#}");
                event_loop.exit();
            }
        }
    }
    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(st) = self.state.as_mut() {
                    st.config.width = size.width.max(1);
                    st.config.height = size.height.max(1);
                    st.surface.configure(&st.device, &st.config);
                    st.size = size;
                }
            }
            WindowEvent::RedrawRequested => self.render(),
            WindowEvent::KeyboardInput { event: KeyEvent { physical_key: PhysicalKey::Code(code), state: ElementState::Pressed, .. }, .. } => {
                match code {
                    KeyCode::Escape => event_loop.exit(),
                    KeyCode::KeyW | KeyCode::ArrowUp => {
                        if let Some(st) = self.state.as_mut() {
                            st.cam_pos.z -= 2.0;
                        }
                    }
                    KeyCode::KeyS | KeyCode::ArrowDown => {
                        if let Some(st) = self.state.as_mut() {
                            st.cam_pos.z += 2.0;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
