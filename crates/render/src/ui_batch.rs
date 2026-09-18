//! UI batch: quads instanciados (SDF rounded-rect) + texto con glyphon.

use anyhow::Result;
use glyphon::{
    fontdb, Attrs, Buffer, Cache, Color, FontSystem, Metrics, Resolution, Shaping, SwashCache,
    TextArea, TextAtlas, TextBounds, TextRenderer, Viewport,
};
use rivaren_core::{DrawList, TextAlign, UiQuad};

pub struct UiBatch {
    quad_pipe: wgpu::RenderPipeline,
    quad_bg: wgpu::BindGroup,
    instance_buf: wgpu::Buffer,
    screen_buf: wgpu::Buffer,
    capacity: usize,
    font_system: FontSystem,
    swash: SwashCache,
    atlas: TextAtlas,
    viewport: Viewport,
    text_renderer: TextRenderer,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    rect: [f32; 4],
    color: [f32; 4],
    params: [f32; 4],
    border_color: [f32; 4],
    clip: [f32; 4],
}

impl From<&UiQuad> for Instance {
    fn from(q: &UiQuad) -> Self {
        Self {
            rect: q.rect,
            color: q.color,
            params: q.params,
            border_color: q.border_color,
            clip: q.clip,
        }
    }
}

impl UiBatch {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ui"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../assets/shaders/ui.wgsl").into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui-bgl"),
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
        let screen_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui-screen"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let quad_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ui-bg"),
            layout: &bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen_buf.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui-layout"),
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let attrs: Vec<wgpu::VertexAttribute> = (0..5)
            .map(|i| wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: i * 16,
                shader_location: i as u32,
            })
            .collect();
        let quad_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui-quads"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Instance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attrs,
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        let capacity = 8192;
        let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui-instances"),
            size: (capacity * std::mem::size_of::<Instance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut db = fontdb::Database::new();
        db.load_font_data(include_bytes!("../../../assets/fonts/RivarenSans.ttf").to_vec());
        db.load_font_data(include_bytes!("../../../assets/fonts/RivarenSans-Bold.ttf").to_vec());
        let font_system = FontSystem::new_with_locale_and_db("es".into(), db);
        let swash = SwashCache::new();
        let cache = Cache::new(device);
        let mut atlas = TextAtlas::new(device, queue, &cache, format);
        let viewport = Viewport::new(device, &cache);
        let text_renderer = TextRenderer::new(&mut atlas, device, Default::default(), None);

        Self {
            quad_pipe,
            quad_bg,
            instance_buf,
            screen_buf,
            capacity,
            font_system,
            swash,
            atlas,
            viewport,
            text_renderer,
        }
    }

    /// Mide el ancho de un texto (para layout responsivo).
    pub fn measure(&mut self, text: &str, size: f32) -> f32 {
        let mut buf = Buffer::new(&mut self.font_system, Metrics::new(size, size * 1.25));
        buf.set_size(&mut self.font_system, Some(10000.0), None);
        let attrs = Attrs::new();
        buf.set_text(&mut self.font_system, text, &attrs, Shaping::Advanced, None);
        buf.shape_until_scroll(&mut self.font_system, false);
        buf.layout_runs().map(|r| r.line_w).fold(0.0, f32::max)
    }

    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        draw: &DrawList,
        size: (u32, u32),
    ) -> Result<()> {
        let instances: Vec<Instance> = draw.quads.iter().map(Instance::from).collect();
        let n = instances.len().min(self.capacity);
        if n > 0 {
            queue.write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(&instances[..n]));
        }
        if std::env::var("RIVAREN_DEBUG").is_ok() {
            use std::sync::atomic::{AtomicU64, Ordering};
            static CTR: AtomicU64 = AtomicU64::new(0);
            let c = CTR.fetch_add(1, Ordering::Relaxed);
            if c % 240 == 0 {
                eprintln!(
                    "[ui] n={} target_size={:?} first={:?} second={:?}",
                    n,
                    size,
                    instances.first().map(|i| i.rect),
                    instances.get(1).map(|i| i.rect)
                );
            }
        }
        let screen = [size.0 as f32, size.1 as f32, 1.0, 0.0];
        queue.write_buffer(&self.screen_buf, 0, bytemuck::cast_slice(&screen));
        self.viewport.update(
            queue,
            Resolution {
                width: size.0,
                height: size.1,
            },
        );

        let mut buffers: Vec<Buffer> = Vec::with_capacity(draw.texts.len());
        for t in &draw.texts {
            let mut buf = Buffer::new(
                &mut self.font_system,
                Metrics::new(t.size, (t.size * 1.25).max(t.size + 2.0)),
            );
            buf.set_size(&mut self.font_system, Some(t.max_width.min(10000.0)), None);
            let attrs = Attrs::new();
            buf.set_text(&mut self.font_system, &t.text, &attrs, Shaping::Advanced, None);
            buf.shape_until_scroll(&mut self.font_system, false);
            buffers.push(buf);
        }
        let areas: Vec<TextArea> = buffers
            .iter()
            .zip(draw.texts.iter())
            .map(|(buf, t)| {
                let line_w = buf.layout_runs().next().map(|r| r.line_w).unwrap_or(0.0);
                let left = match t.align {
                    TextAlign::Left => t.x,
                    TextAlign::Center => t.x - line_w * 0.5,
                    TextAlign::Right => t.x - line_w,
                };
                TextArea {
                    buffer: buf,
                    left,
                    top: t.y,
                    scale: 1.0,
                    bounds: TextBounds {
                        left: t.clip[0] as i32,
                        top: t.clip[1] as i32,
                        right: t.clip[2] as i32,
                        bottom: t.clip[3] as i32,
                    },
                    default_color: Color::rgba(
                        (t.color[0] * 255.0) as u8,
                        (t.color[1] * 255.0) as u8,
                        (t.color[2] * 255.0) as u8,
                        (t.color[3] * 255.0) as u8,
                    ),
                    custom_glyphs: &[],
                }
            })
            .collect();
        self.text_renderer.prepare(
            device,
            queue,
            &mut self.font_system,
            &mut self.atlas,
            &self.viewport,
            areas,
            &mut self.swash,
        )?;

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("ui-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if n > 0 {
            pass.set_pipeline(&self.quad_pipe);
            pass.set_bind_group(0, &self.quad_bg, &[]);
            pass.set_vertex_buffer(0, self.instance_buf.slice(..));
            pass.draw(0..6, 0..n as u32);
        }
        self.text_renderer.render(&self.atlas, &self.viewport, &mut pass)?;
        drop(pass);
        self.atlas.trim();
        Ok(())
    }
}
