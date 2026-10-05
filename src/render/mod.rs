//! GPU renderer: batched quads (solid, image, rounded, bordered) interleaved
//! with glyphon text layers so text and shapes keep their painter's order.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use glyphon::{
    Cache, ColorMode, Resolution, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer,
    Viewport,
};
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::assets::Assets;
use crate::ui::desc::Color;
use crate::ui::{DrawItem, Quad, Rect, TextDraw, Ui};

/// Textures unused for this long are released.
const TEXTURE_IDLE_LIMIT: Duration = Duration::from_secs(60);

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct QuadInstance {
    rect: [f32; 4],
    uv: [f32; 4],
    color: [f32; 4],
    border_color: [f32; 4],
    params: [f32; 4],
}

struct Texture {
    bind_group: wgpu::BindGroup,
    last_used: Instant,
}

/// Quads followed by text; a new layer starts whenever a quad follows text.
#[derive(Default)]
struct Layer {
    batches: Vec<(Option<String>, std::ops::Range<u32>)>,
    texts: Vec<TextDraw>,
}

/// Where frames go: a window surface, or a texture for headless rendering.
enum Target {
    Window {
        window: Arc<Window>,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
    },
    Offscreen {
        texture: wgpu::Texture,
    },
}

const OFFSCREEN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub struct Renderer {
    target: Target,
    /// Format of the views rendered into (always non-sRGB).
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    globals_buffer: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    white: wgpu::BindGroup,
    textures: HashMap<String, Option<Texture>>,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    atlas: TextAtlas,
    viewport: Viewport,
    swash: SwashCache,
    text_renderers: Vec<TextRenderer>,
}

impl Renderer {
    pub async fn for_window(instance: &wgpu::Instance, window: Arc<Window>) -> Result<Self> {
        let size = window.inner_size();
        let surface = instance
            .create_surface(window.clone())
            .context("create surface")?;
        let (adapter, device, queue) = request_device(instance, Some(&surface)).await?;
        let caps = surface.get_capabilities(&adapter);
        // Colors are blended in sRGB space, like browsers and most UI toolkits,
        // so translucent UI looks the way it was designed.
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0].remove_srgb_suffix());
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: if caps.formats.contains(&format) {
                format
            } else {
                caps.formats[0]
            },
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: if caps.formats.contains(&format) {
                vec![]
            } else {
                vec![format]
            },
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);
        let (width, height) = (config.width, config.height);
        let target = Target::Window {
            window,
            surface,
            config,
        };
        Ok(Self::build(device, queue, format, target, width, height))
    }

    /// A renderer drawing into a texture, for tests and screenshots.
    pub async fn offscreen(width: u32, height: u32) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let (_, device, queue) = request_device(&instance, None).await?;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let target = Target::Offscreen { texture };
        Ok(Self::build(
            device,
            queue,
            OFFSCREEN_FORMAT,
            target,
            width,
            height,
        ))
    }

    fn build(
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
        target: Target,
        width: u32,
        height: u32,
    ) -> Self {
        let globals_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
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
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals_buffer.as_entire_binding(),
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture"),
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
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("quad.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("quad"),
            bind_group_layouts: &[Some(&globals_layout), Some(&texture_layout)],
            ..Default::default()
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quad"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<QuadInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let white =
            Self::upload_texture(&device, &queue, &texture_layout, &sampler, 1, 1, &[255; 4]);
        let instance_capacity = 256;
        let instance_buffer = Self::create_instance_buffer(&device, instance_capacity);

        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let atlas = TextAtlas::with_color_mode(&device, &queue, &cache, format, ColorMode::Web);

        Renderer {
            target,
            format,
            width,
            height,
            device,
            queue,
            pipeline,
            globals_buffer,
            globals_bind_group,
            texture_layout,
            sampler,
            white,
            textures: HashMap::new(),
            instance_buffer,
            instance_capacity,
            atlas,
            viewport,
            swash: SwashCache::new(),
            text_renderers: Vec::new(),
        }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if let Target::Window {
            surface, config, ..
        } = &mut self.target
        {
            config.width = width;
            config.height = height;
            surface.configure(&self.device, config);
            self.width = width;
            self.height = height;
        }
    }

    /// Reads back the last frame of an offscreen renderer.
    pub fn capture(&self) -> Result<image::RgbaImage> {
        let Target::Offscreen { texture } = &self.target else {
            anyhow::bail!("capture is only supported for offscreen renderers");
        };
        let (width, height) = (self.width, self.height);
        let unpadded = width * 4;
        let padded = unpadded.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (padded * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely())?;
        let data = slice
            .get_mapped_range()
            .map_err(|e| anyhow::anyhow!("map readback buffer: {e:?}"))?;
        let mut pixels = Vec::with_capacity((unpadded * height) as usize);
        for row in data.chunks(padded as usize) {
            pixels.extend_from_slice(&row[..unpadded as usize]);
        }
        image::RgbaImage::from_raw(width, height, pixels).context("readback size mismatch")
    }

    fn create_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quad instances"),
            size: (capacity * std::mem::size_of::<QuadInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn upload_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> wgpu::BindGroup {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            rgba,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    /// Makes sure the texture for `src` is resident. Returns false if it cannot be loaded.
    fn ensure_texture(&mut self, src: &str, assets: &Assets, now: Instant) -> bool {
        if !self.textures.contains_key(src) {
            let limit = self.device.limits().max_texture_dimension_2d;
            let texture = assets.load_image(src).and_then(|img| {
                let (w, h) = img.dimensions();
                if w > limit || h > limit {
                    eprintln!(
                        "[deflorta] image '{src}' ({w}x{h}) exceeds the GPU limit of {limit}px"
                    );
                    return None;
                }
                let bind_group = Self::upload_texture(
                    &self.device,
                    &self.queue,
                    &self.texture_layout,
                    &self.sampler,
                    w,
                    h,
                    img.as_raw(),
                );
                Some(Texture {
                    bind_group,
                    last_used: now,
                })
            });
            self.textures.insert(src.to_owned(), texture);
        }
        match self.textures.get_mut(src) {
            Some(Some(texture)) => {
                texture.last_used = now;
                true
            }
            _ => false,
        }
    }

    pub fn render(
        &mut self,
        items: Vec<DrawItem>,
        ui: &mut Ui,
        assets: &Assets,
        clear: Color,
    ) -> Result<()> {
        let now = Instant::now();
        let viewport_rect = ui.viewport();

        // The game area is cleared to the configured color; letterbox bars stay black.
        let background = DrawItem::Quad(Quad {
            rect: viewport_rect,
            color: clear,
            radius: 0.0,
            border_width: 0.0,
            border_color: Color::default(),
            image: None,
        });

        // Split the draw list into layers and collect quad instances.
        let mut instances: Vec<QuadInstance> = Vec::new();
        let mut layers: Vec<Layer> = vec![Layer::default()];
        for item in std::iter::once(background).chain(items) {
            match item {
                DrawItem::Quad(quad) => {
                    if !layers.last().unwrap().texts.is_empty() {
                        layers.push(Layer::default());
                    }
                    let texture = match &quad.image {
                        Some(image) if self.ensure_texture(&image.src, assets, now) => {
                            Some(image.src.clone())
                        }
                        Some(_) => continue,
                        None => None,
                    };
                    let index = instances.len() as u32;
                    instances.push(quad_instance(&quad));
                    let layer = layers.last_mut().unwrap();
                    match layer.batches.last_mut() {
                        Some((t, range)) if *t == texture && range.end == index => range.end += 1,
                        _ => layer.batches.push((texture, index..index + 1)),
                    }
                }
                DrawItem::Text(text) => layers.last_mut().unwrap().texts.push(text),
            }
        }

        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len().next_power_of_two();
            self.instance_buffer =
                Self::create_instance_buffer(&self.device, self.instance_capacity);
        }
        if !instances.is_empty() {
            self.queue
                .write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        }
        let (width, height) = (self.width, self.height);
        self.queue.write_buffer(
            &self.globals_buffer,
            0,
            bytemuck::cast_slice(&[width as f32, height as f32, 0.0, 0.0]),
        );

        // Prepare text, one glyphon renderer per layer.
        self.viewport
            .update(&self.queue, Resolution { width, height });
        let text_layers = layers.iter().filter(|l| !l.texts.is_empty()).count();
        while self.text_renderers.len() < text_layers {
            self.text_renderers.push(TextRenderer::new(
                &mut self.atlas,
                &self.device,
                wgpu::MultisampleState::default(),
                None,
            ));
        }
        let bounds = TextBounds {
            left: viewport_rect.x as i32,
            top: viewport_rect.y as i32,
            right: (viewport_rect.x + viewport_rect.w) as i32,
            bottom: (viewport_rect.y + viewport_rect.h) as i32,
        };
        let (font_system, text_entries) = ui.text.split();
        let mut renderer_index = 0;
        for layer in &layers {
            if layer.texts.is_empty() {
                continue;
            }
            let areas = layer.texts.iter().filter_map(|t| {
                let entry = text_entries.get(&t.id)?;
                let [r, g, b, a] = t.color.to_rgba8();
                Some(TextArea {
                    buffer: &entry.buffer,
                    left: t.x,
                    top: t.y,
                    scale: t.scale,
                    bounds,
                    default_color: glyphon::Color::rgba(r, g, b, a),
                    custom_glyphs: &[],
                })
            });
            self.text_renderers[renderer_index].prepare(
                &self.device,
                &self.queue,
                font_system,
                &mut self.atlas,
                &self.viewport,
                areas,
                &mut self.swash,
            )?;
            renderer_index += 1;
        }

        // A suboptimal frame is still drawn and presented; the surface may only be
        // reconfigured once that frame has been released.
        let mut reconfigure = false;
        let (frame, view) = match &mut self.target {
            Target::Window {
                window,
                surface,
                config,
            } => {
                let frame = match surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(frame) => frame,
                    wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                        reconfigure = true;
                        frame
                    }
                    wgpu::CurrentSurfaceTexture::Timeout
                    | wgpu::CurrentSurfaceTexture::Occluded => {
                        return Ok(());
                    }
                    wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                        surface.configure(&self.device, config);
                        window.request_redraw();
                        return Ok(());
                    }
                    wgpu::CurrentSurfaceTexture::Validation => {
                        anyhow::bail!("surface validation error")
                    }
                };
                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
                    format: Some(self.format),
                    ..Default::default()
                });
                (Some(frame), view)
            }
            Target::Offscreen { texture } => (None, texture.create_view(&Default::default())),
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
            let (sx, sy, sw, sh) = scissor(viewport_rect, width, height);
            pass.set_scissor_rect(sx, sy, sw, sh);
            let mut renderer_index = 0;
            for layer in &layers {
                for (texture, range) in &layer.batches {
                    let bind_group = match texture {
                        Some(src) => match self.textures.get(src) {
                            Some(Some(t)) => &t.bind_group,
                            _ => continue,
                        },
                        None => &self.white,
                    };
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, &self.globals_bind_group, &[]);
                    pass.set_bind_group(1, bind_group, &[]);
                    pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
                    pass.draw(0..4, range.clone());
                }
                if !layer.texts.is_empty() {
                    self.text_renderers[renderer_index].render(
                        &self.atlas,
                        &self.viewport,
                        &mut pass,
                    )?;
                    renderer_index += 1;
                }
            }
        }
        self.queue.submit(Some(encoder.finish()));
        if let (
            Some(frame),
            Target::Window {
                window,
                surface,
                config,
            },
        ) = (frame, &mut self.target)
        {
            window.pre_present_notify();
            self.queue.present(frame);
            if reconfigure {
                let size = window.inner_size();
                if size.width > 0 && size.height > 0 {
                    config.width = size.width;
                    config.height = size.height;
                    self.width = size.width;
                    self.height = size.height;
                }
                surface.configure(&self.device, config);
            }
        }
        self.atlas.trim();
        self.textures.retain(|_, t| {
            t.as_ref()
                .is_none_or(|t| now.duration_since(t.last_used) < TEXTURE_IDLE_LIMIT)
        });
        Ok(())
    }
}

async fn request_device(
    instance: &wgpu::Instance,
    surface: Option<&wgpu::Surface<'_>>,
) -> Result<(wgpu::Adapter, wgpu::Device, wgpu::Queue)> {
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: surface,
            ..Default::default()
        })
        .await
        .context("no compatible GPU adapter")?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("deflorta"),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adapter.limits()),
            ..Default::default()
        })
        .await
        .context("request device")?;
    Ok((adapter, device, queue))
}

fn scissor(rect: Rect, width: u32, height: u32) -> (u32, u32, u32, u32) {
    let x = (rect.x.max(0.0) as u32).min(width);
    let y = (rect.y.max(0.0) as u32).min(height);
    let w = ((rect.x + rect.w).ceil().max(0.0) as u32)
        .min(width)
        .saturating_sub(x)
        .max(1);
    let h = ((rect.y + rect.h).ceil().max(0.0) as u32)
        .min(height)
        .saturating_sub(y)
        .max(1);
    (x, y, w, h)
}

fn quad_instance(quad: &Quad) -> QuadInstance {
    let uv = quad.image.as_ref().map_or([0.0, 0.0, 1.0, 1.0], |i| i.uv);
    QuadInstance {
        rect: [quad.rect.x, quad.rect.y, quad.rect.w, quad.rect.h],
        uv,
        color: quad.color.0,
        border_color: quad.border_color.0,
        params: [quad.radius, quad.border_width, 0.0, 0.0],
    }
}
