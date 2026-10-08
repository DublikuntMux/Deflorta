use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use bytemuck::{Pod, Zeroable};
use glyphon::{
    Cache, ColorMode, Resolution, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer,
    Viewport,
};
use log::{debug, error, info, trace, warn};
use num_traits::AsPrimitive;
use wgpu::util::DeviceExt;
use wgpu::{CommandEncoderDescriptor, PipelineCompilationOptions, TextureViewDescriptor};
use winit::window::Window;

use crate::assets::{ASSET_IDLE_LIMIT, Assets};
use crate::ui::desc::Color;
use crate::ui::{DrawItem, ImageRef, Quad, Rect, TextDraw, Ui};
use crate::util::math::{clamp_to_u32, saturating_i32};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct QuadInstance {
    rect: [f32; 4],
    uv: [f32; 4],
    color: [f32; 4],
    border_color: [f32; 4],
    params: [f32; 4],
    mask: [f32; 4],
    radii: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    screen: [f32; 2],
    _pad: [f32; 2],
    viewport: [f32; 4],
}

struct Texture {
    raw: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    serial: u64,
}

struct CachedTexture {
    texture: Option<Texture>,
    last_used: Instant,
}

/// Draw batches share a texture, a mask texture and a clip rectangle.
#[derive(Clone, PartialEq)]
struct BatchKey {
    texture: Option<String>,
    mask: Option<String>,
    clip: [u32; 4],
}

/// Quads followed by text; a new layer starts whenever a quad follows text.
#[derive(Default)]
struct Layer {
    batches: Vec<(BatchKey, std::ops::Range<u32>)>,
    texts: Vec<TextDraw>,
}

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
    textures: HashMap<String, CachedTexture>,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    atlas: TextAtlas,
    viewport: Viewport,
    swash: SwashCache,
    text_renderers: Vec<TextRenderer>,
}

impl Renderer {
    #[cfg(feature = "dev-console")]
    pub fn loaded_assets(&self) -> Vec<crate::dev_console::diagnostics::LoadedAsset> {
        self.textures
            .iter()
            .filter_map(|(source, texture)| {
                let texture = texture.texture.as_ref()?;
                let (w, h) = (texture.raw.width(), texture.raw.height());
                Some(crate::dev_console::diagnostics::LoadedAsset {
                    kind: "GPU texture",
                    source: source.clone(),
                    state: "Resident".into(),
                    detail: format!("{w}×{h} RGBA8"),
                    bytes: Some(u64::from(w) * u64::from(h) * 4),
                })
            })
            .collect()
    }

    #[cfg(feature = "dev-console")]
    pub fn diagnostic_stats(&self) -> crate::dev_console::diagnostics::GpuStats {
        let info = self.device.adapter_info();
        let counters = self.device.get_internal_counters();
        // wgpu 30 maintains memory counters on Vulkan and DirectX 12 only.
        let memory_counters = matches!(info.backend, wgpu::Backend::Vulkan | wgpu::Backend::Dx12);
        let assets = self.loaded_assets();
        crate::dev_console::diagnostics::GpuStats {
            adapter: format!(
                "{} · {:?} · {:?}",
                info.name, info.backend, info.device_type
            ),
            textures: assets.len(),
            texture_bytes: assets.iter().filter_map(|asset| asset.bytes).sum(),
            buffer_bytes: memory_counters.then(|| counters.hal.buffer_memory.read()),
            all_texture_bytes: memory_counters.then(|| counters.hal.texture_memory.read()),
            allocations: self
                .device
                .generate_allocator_report()
                .map(|report| (report.total_allocated_bytes, report.total_reserved_bytes)),
        }
    }

    #[cfg(feature = "dev-console")]
    pub fn create_console(&self, window: &Window) -> crate::dev_console::DevConsole {
        crate::dev_console::DevConsole::new(window, &self.device, self.format)
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the surface, GPU adapter, or rendering device cannot be created.
    pub async fn for_window(instance: &wgpu::Instance, window: Arc<Window>) -> Result<Self> {
        let size = window.inner_size();
        let surface = instance
            .create_surface(window.clone())
            .context("create surface")?;
        let (adapter, device, queue) = request_device(instance, Some(&surface)).await?;
        let caps = surface.get_capabilities(&adapter);
        // 8-bit non-sRGB targets: colors blend in sRGB space like browsers, and
        // frames can be read back as RGBA8 for thumbnails. Drivers may list
        // float or 10-bit formats first, so pick explicitly.
        let format = [
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8Unorm,
        ]
        .into_iter()
        .find(|f| caps.formats.contains(f))
        .unwrap_or_else(|| caps.formats[0].remove_srgb_suffix());
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
        info!(
            "Window surface {width}x{height}, format {:?} (render {format:?}), present {:?}, scale {:.2}",
            config.format,
            config.present_mode,
            window.scale_factor()
        );
        debug!("Surface formats offered: {:?}", caps.formats);
        let target = Target::Window {
            window,
            surface,
            config,
        };
        Ok(Self::build(device, queue, format, target, width, height))
    }

    ///
    /// # Errors
    ///
    /// Returns an error if no suitable GPU adapter or rendering device is available.
    pub async fn offscreen(width: u32, height: u32) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let (_, device, queue) = request_device(&instance, None).await?;
        let texture = create_target_texture(&device, OFFSCREEN_FORMAT, width, height);
        info!("Offscreen target {width}x{height}, format {OFFSCREEN_FORMAT:?}");
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
            size: u64::try_from(std::mem::size_of::<Globals>()).expect("uniform size fits u64"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
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

        let pipeline =
            Self::create_quad_pipeline(&device, format, &globals_layout, &texture_layout);

        let (_, white) =
            Self::upload_texture(&device, &queue, &texture_layout, &sampler, 1, 1, &[255; 4]);
        let instance_capacity = 256;
        let instance_buffer = Self::create_instance_buffer(&device, instance_capacity);

        let cache = Cache::new(&device);
        let viewport = Viewport::new(&device, &cache);
        let atlas = TextAtlas::with_color_mode(&device, &queue, &cache, format, ColorMode::Web);

        Self {
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

    fn create_quad_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        globals_layout: &wgpu::BindGroupLayout,
        texture_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("quad"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(wgpu::include_spirv_source!(concat!(
                env!("OUT_DIR"),
                "/shaders/quad.spv"
            )))),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("quad"),
            bind_group_layouts: &[
                Some(globals_layout),
                Some(texture_layout),
                Some(texture_layout),
            ],
            ..Default::default()
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("quad"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: u64::try_from(std::mem::size_of::<QuadInstance>())
                        .expect("quad stride fits u64"),
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![
                        0 => Float32x4, 1 => Float32x4, 2 => Float32x4,
                        3 => Float32x4, 4 => Float32x4, 5 => Float32x4,
                        6 => Float32x4
                    ],
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    pub const fn size(&self) -> (u32, u32) {
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
            debug!("Surface resized to {width}x{height}");
        }
    }

    fn create_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quad instances"),
            size: u64::try_from(capacity * std::mem::size_of::<QuadInstance>())
                .expect("instance buffer size fits u64"),
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
    ) -> (wgpu::Texture, wgpu::BindGroup) {
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
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            rgba,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
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
        });
        (texture, bind_group)
    }

    fn texture_from_pixels(&self, img: &image::RgbaImage, serial: u64) -> Option<Texture> {
        let (w, h) = img.dimensions();
        let limit = self.device.limits().max_texture_dimension_2d;
        if w == 0 || h == 0 || w > limit || h > limit {
            warn!("Image of {w}x{h} exceeds the GPU limit of {limit}px");
            return None;
        }
        trace!("Uploading {w}x{h} texture");
        let (texture, bind_group) = Self::upload_texture(
            &self.device,
            &self.queue,
            &self.texture_layout,
            &self.sampler,
            w,
            h,
            img.as_raw(),
        );
        Some(Texture {
            raw: texture,
            bind_group,
            serial,
        })
    }

    fn ensure_texture(&mut self, image: &ImageRef, assets: &mut Assets, now: Instant) -> bool {
        let Some((serial, frame)) = &image.frame else {
            return self.ensure_image(&image.src, assets, now);
        };
        let reusable = self
            .textures
            .get(&image.src)
            .and_then(|cached| cached.texture.as_ref())
            .is_some_and(|texture| {
                texture.raw.width() == frame.width() && texture.raw.height() == frame.height()
            });
        if !reusable {
            let texture = self.texture_from_pixels(frame, *serial);
            self.textures.insert(
                image.src.clone(),
                CachedTexture {
                    texture,
                    last_used: now,
                },
            );
        }
        let Some(cached) = self.textures.get_mut(&image.src) else {
            return false;
        };
        cached.last_used = now;
        let Some(texture) = cached.texture.as_mut() else {
            return false;
        };
        if texture.serial != *serial {
            texture.serial = *serial;
            self.queue.write_texture(
                texture.raw.as_image_copy(),
                frame.as_raw(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(frame.width() * 4),
                    rows_per_image: None,
                },
                texture.raw.size(),
            );
        }
        true
    }

    fn ensure_image(&mut self, src: &str, assets: &mut Assets, now: Instant) -> bool {
        assets.request(src);
        if !self.textures.contains_key(src) {
            match assets.take_pixels(src) {
                Some(img) => {
                    let texture = self.texture_from_pixels(&img, 0);
                    self.textures.insert(
                        src.to_owned(),
                        CachedTexture {
                            texture,
                            last_used: now,
                        },
                    );
                }
                None if assets.is_settled(src) => {
                    self.textures.insert(
                        src.to_owned(),
                        CachedTexture {
                            texture: None,
                            last_used: now,
                        },
                    );
                }
                None => return false,
            }
        }
        match self.textures.get_mut(src) {
            Some(cached) => {
                cached.last_used = now;
                cached.texture.is_some()
            }
            _ => false,
        }
    }

    fn prepare(
        &mut self,
        items: Vec<DrawItem>,
        ui: &mut Ui,
        assets: &mut Assets,
        clear: Color,
    ) -> Result<Vec<Layer>> {
        let now = Instant::now();
        let viewport_rect = ui.viewport();
        let (width, height) = (self.width, self.height);
        let full = Rect {
            x: 0.0,
            y: 0.0,
            w: width.as_(),
            h: height.as_(),
        };

        let background = DrawItem::Quad(Quad {
            rect: viewport_rect,
            rotation: 0.0,
            color: clear,
            radii: [0.0; 4],
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
            image: None,
            clip: viewport_rect,
            mask: None,
        });

        let mut instances: Vec<QuadInstance> = Vec::new();
        let mut layers: Vec<Layer> = vec![Layer::default()];
        for item in std::iter::once(background).chain(items) {
            match item {
                DrawItem::Quad(quad) => {
                    if !layers.last().unwrap().texts.is_empty() {
                        layers.push(Layer::default());
                    }
                    let texture = match &quad.image {
                        Some(image) if self.ensure_texture(image, assets, now) => {
                            Some(image.src.clone())
                        }
                        Some(_) => continue,
                        None => None,
                    };
                    let mask = match quad.mask.as_ref().and_then(|m| m.src.clone()) {
                        Some(src) if self.ensure_image(&src, assets, now) => Some(src),
                        Some(_) => continue,
                        None => None,
                    };
                    let Some(clip) = scissor(quad.clip.intersect(&full), width, height) else {
                        continue;
                    };
                    let key = BatchKey {
                        texture,
                        mask,
                        clip,
                    };
                    let index =
                        u32::try_from(instances.len()).context("too many quad instances")?;
                    let end = index.checked_add(1).context("too many quad instances")?;
                    instances.push(quad_instance(&quad));
                    let layer = layers.last_mut().unwrap();
                    match layer.batches.last_mut() {
                        Some((k, range)) if *k == key && range.end == index => range.end = end,
                        _ => layer.batches.push((key, index..end)),
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
        let globals = Globals {
            screen: [width.as_(), height.as_()],
            _pad: [0.0; 2],
            viewport: [
                viewport_rect.x,
                viewport_rect.y,
                viewport_rect.w,
                viewport_rect.h,
            ],
        };
        self.queue
            .write_buffer(&self.globals_buffer, 0, bytemuck::bytes_of(&globals));

        self.prepare_text(&layers, ui, full)?;
        Ok(layers)
    }

    fn prepare_text(&mut self, layers: &[Layer], ui: &mut Ui, full: Rect) -> Result<()> {
        self.viewport.update(
            &self.queue,
            Resolution {
                width: self.width,
                height: self.height,
            },
        );
        let text_layers = layers.iter().filter(|l| !l.texts.is_empty()).count();
        while self.text_renderers.len() < text_layers {
            self.text_renderers.push(TextRenderer::new(
                &mut self.atlas,
                &self.device,
                wgpu::MultisampleState::default(),
                None,
            ));
        }
        let (font_system, text_entries) = ui.text.split();
        let mut renderer_index = 0;
        for layer in layers {
            if layer.texts.is_empty() {
                continue;
            }
            let areas = layer.texts.iter().filter_map(|t| {
                let entry = text_entries.get(&t.id)?;
                let [r, g, b, a] = t.color.to_rgba8();
                let clip = t.clip.intersect(&full);
                Some(TextArea {
                    buffer: &entry.buffer,
                    left: t.x,
                    top: t.y,
                    scale: t.scale,
                    bounds: TextBounds {
                        left: saturating_i32(clip.x.floor()),
                        top: saturating_i32(clip.y.floor()),
                        right: saturating_i32((clip.x + clip.w).ceil()),
                        bottom: saturating_i32((clip.y + clip.h).ceil()),
                    },
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
        Ok(())
    }

    fn encode(&self, view: &wgpu::TextureView, layers: &[Layer]) -> Result<wgpu::CommandBuffer> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            let bind_group = |key: &Option<String>| {
                key.as_ref().map_or(Some(&self.white), |src| {
                    self.textures
                        .get(src)
                        .and_then(|t| t.texture.as_ref())
                        .map(|t| &t.bind_group)
                })
            };
            let mut renderer_index = 0;
            for layer in layers {
                for (key, range) in &layer.batches {
                    let (Some(texture), Some(mask)) =
                        (bind_group(&key.texture), bind_group(&key.mask))
                    else {
                        continue;
                    };
                    let [x, y, w, h] = key.clip;
                    pass.set_scissor_rect(x, y, w, h);
                    pass.set_pipeline(&self.pipeline);
                    pass.set_bind_group(0, &self.globals_bind_group, &[]);
                    pass.set_bind_group(1, texture, &[]);
                    pass.set_bind_group(2, mask, &[]);
                    pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
                    pass.draw(0..4, range.clone());
                }
                if !layer.texts.is_empty() {
                    pass.set_scissor_rect(0, 0, self.width, self.height);
                    self.text_renderers[renderer_index].render(
                        &self.atlas,
                        &self.viewport,
                        &mut pass,
                    )?;
                    renderer_index += 1;
                }
            }
        }
        Ok(encoder.finish())
    }

    pub fn unload_texture(&mut self, src: &str) -> bool {
        self.textures.remove(src).is_some()
    }

    pub fn collect_unused(
        &mut self,
        assets: &mut Assets,
        retained: &HashSet<String>,
        now: Instant,
    ) {
        for src in assets.collect_unused(retained, now) {
            self.unload_texture(&src);
        }
        self.textures.retain(|src, t| {
            let keep = retained.contains(src)
                || now.saturating_duration_since(t.last_used) < ASSET_IDLE_LIMIT;
            if !keep {
                assets.forget(src);
            }
            keep
        });
    }

    ///
    /// # Errors
    ///
    /// Returns an error if text preparation, command encoding, or acquiring the window surface fails.
    pub fn render(
        &mut self,
        items: Vec<DrawItem>,
        ui: &mut Ui,
        assets: &mut Assets,
        clear: Color,
        #[cfg(feature = "dev-console")] mut console: Option<&mut crate::dev_console::DevConsole>,
    ) -> Result<bool> {
        let layers = self.prepare(items, ui, assets, clear)?;
        #[cfg(feature = "dev-console")]
        if let Some(console) = &mut console {
            console.upload_textures(&self.device, &self.queue);
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
                        debug!("Surface suboptimal; reconfiguring after this frame");
                        reconfigure = true;
                        frame
                    }
                    wgpu::CurrentSurfaceTexture::Timeout => {
                        debug!("Timed out acquiring a frame; skipping it");
                        return Ok(false);
                    }
                    wgpu::CurrentSurfaceTexture::Occluded => return Ok(false),
                    status @ (wgpu::CurrentSurfaceTexture::Outdated
                    | wgpu::CurrentSurfaceTexture::Lost) => {
                        let kind = if matches!(status, wgpu::CurrentSurfaceTexture::Lost) {
                            "lost"
                        } else {
                            "outdated"
                        };
                        debug!("Surface {kind}; reconfiguring");
                        surface.configure(&self.device, config);
                        window.request_redraw();
                        return Ok(false);
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
            Target::Offscreen { texture } => {
                (None, texture.create_view(&TextureViewDescriptor::default()))
            }
        };
        let commands = self.encode(&view, &layers)?;
        self.queue.submit(Some(commands));
        #[cfg(feature = "dev-console")]
        if let Some(console) = console {
            let commands =
                console.paint(&self.device, &self.queue, &view, [self.width, self.height]);
            self.queue.submit(commands);
        }
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
        Ok(true)
    }

    ///
    /// # Errors
    ///
    /// Returns an error if preparing, rendering, or reading back the frame fails.
    pub fn render_to_image(
        &mut self,
        items: Vec<DrawItem>,
        ui: &mut Ui,
        assets: &mut Assets,
        clear: Color,
    ) -> Result<image::RgbaImage> {
        let layers = self.prepare(items, ui, assets, clear)?;
        let texture = create_target_texture(&self.device, self.format, self.width, self.height);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let commands = self.encode(&view, &layers)?;
        self.queue.submit(Some(commands));
        self.read_texture(&texture)
    }

    ///
    /// # Errors
    ///
    /// Returns an error for window-backed renderers, unsupported texture formats, or GPU readback failures.
    pub fn capture(&self) -> Result<image::RgbaImage> {
        let Target::Offscreen { texture } = &self.target else {
            anyhow::bail!("capture is only supported for offscreen renderers");
        };
        self.read_texture(texture)
    }

    fn read_texture(&self, texture: &wgpu::Texture) -> Result<image::RgbaImage> {
        let format = texture.format();
        if format.block_copy_size(None) != Some(4) || format.components() != 4 {
            anyhow::bail!("cannot read back frames in {format:?}");
        }
        let (width, height) = (texture.width(), texture.height());
        let unpadded = width * 4;
        let padded = unpadded.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor::default());
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
        let bgra = matches!(
            texture.format(),
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let pixel_bytes = usize::try_from(u64::from(unpadded) * u64::from(height))
            .context("readback pixel buffer exceeds addressable memory")?;
        let padded = usize::try_from(padded).context("readback row exceeds addressable memory")?;
        let unpadded =
            usize::try_from(unpadded).context("readback row exceeds addressable memory")?;
        let mut pixels = Vec::with_capacity(pixel_bytes);
        for row in data.chunks(padded) {
            let row = &row[..unpadded];
            if bgra {
                for px in row.chunks(4) {
                    pixels.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
                }
            } else {
                pixels.extend_from_slice(row);
            }
        }
        image::RgbaImage::from_raw(width, height, pixels).context("readback size mismatch")
    }
}

fn create_target_texture(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
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
    let info = adapter.get_info();
    info!(
        "GPU: {} ({:?}, {:?}), driver {} {}",
        info.name, info.device_type, info.backend, info.driver, info.driver_info
    );
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("deflorta"),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adapter.limits()),
            ..Default::default()
        })
        .await
        .context("request device")?;
    device.on_uncaptured_error(Arc::new(|err| error!("GPU error: {err}")));
    device.set_device_lost_callback(|reason, message| {
        if reason != wgpu::DeviceLostReason::Destroyed {
            error!("GPU device lost ({reason:?}): {message}");
        }
    });
    Ok((adapter, device, queue))
}

/// Integer scissor rectangle, or None when empty.
fn scissor(rect: Rect, width: u32, height: u32) -> Option<[u32; 4]> {
    let x = clamp_to_u32(rect.x.floor(), width);
    let y = clamp_to_u32(rect.y.floor(), height);
    let x1 = clamp_to_u32((rect.x + rect.w).ceil(), width);
    let y1 = clamp_to_u32((rect.y + rect.h).ceil(), height);
    (x1 > x && y1 > y).then(|| [x, y, x1 - x, y1 - y])
}

fn quad_instance(quad: &Quad) -> QuadInstance {
    let uv = quad.image.as_ref().map_or([0.0, 0.0, 1.0, 1.0], |i| i.uv);
    let (mask, invert) = quad.mask.as_ref().map_or(([0.0; 4], 0.0), |m| {
        (
            [m.kind.as_(), m.progress, m.param, 0.0],
            f32::from(m.invert),
        )
    });
    QuadInstance {
        rect: [quad.rect.x, quad.rect.y, quad.rect.w, quad.rect.h],
        uv,
        color: quad.color.0,
        border_color: quad.border_color.0,
        params: [quad.border_width, quad.rotation, invert, 0.0],
        mask,
        radii: quad.radii,
    }
}

#[cfg(test)]
mod tests {
    use super::{Rect, scissor};

    mod notifications;

    #[test]
    #[ignore = "Requires a native GPU adapter"]
    fn cleanup_releases_cpu_gpu_and_failed_video_entries_and_images_reload() {
        use super::*;
        use std::time::Duration;

        let files = crate::GameFiles::open(&crate::workspace_dir().join("game")).unwrap();
        let mut assets = Assets::new(files);
        let mut renderer = pollster::block_on(Renderer::offscreen(32, 32)).unwrap();
        let source = "images/masks/clouds.png";
        let wait_for_image = |assets: &mut Assets| {
            assets.request(source);
            let deadline = Instant::now() + Duration::from_secs(5);
            while !assets.is_settled(source) {
                assert!(Instant::now() < deadline, "image decoding timed out");
                assets.poll();
                std::thread::sleep(Duration::from_millis(1));
            }
        };
        wait_for_image(&mut assets);
        assert!(renderer.ensure_image(source, &mut assets, Instant::now()));
        let now = Instant::now();
        let retained = HashSet::from([source.to_owned()]);
        renderer.collect_unused(&mut assets, &retained, now + ASSET_IDLE_LIMIT);
        assert!(renderer.textures.contains_key(source));
        assert!(assets.is_settled(source));
        renderer.collect_unused(&mut assets, &HashSet::new(), now + ASSET_IDLE_LIMIT * 2);
        assert!(!renderer.textures.contains_key(source));
        assert!(!assets.is_settled(source));

        wait_for_image(&mut assets);
        assert!(renderer.ensure_image(source, &mut assets, Instant::now()));
        assert!(renderer.unload_texture(source));
        assert!(assets.forget(source));
        wait_for_image(&mut assets);
        assert!(renderer.ensure_image(source, &mut assets, Instant::now()));

        let invalid = ImageRef {
            src: "video:/invalid".into(),
            uv: [0.0; 4],
            frame: Some((1, Arc::new(image::RgbaImage::new(0, 0)))),
        };
        assert!(!renderer.ensure_texture(&invalid, &mut assets, now));
        assert!(renderer.textures.contains_key(&invalid.src));
        renderer.collect_unused(&mut assets, &retained, now + ASSET_IDLE_LIMIT);
        assert!(!renderer.textures.contains_key(&invalid.src));
        assert!(renderer.textures.contains_key(source));
    }

    #[test]
    fn scissor_rounds_outward_and_clips_to_surface() {
        let rect = Rect {
            x: -2.5,
            y: 1.5,
            w: 10.0,
            h: 20.0,
        };
        assert_eq!(scissor(rect, 10, 10), Some([0, 1, 8, 9]));
    }

    #[test]
    fn scissor_handles_coordinates_outside_integer_range() {
        let rect = Rect {
            x: 1.0,
            y: 2.0,
            w: f32::MAX,
            h: f32::INFINITY,
        };
        assert_eq!(scissor(rect, 10, 10), Some([1, 2, 9, 8]));
        assert_eq!(
            scissor(
                Rect {
                    x: f32::MAX,
                    ..rect
                },
                10,
                10
            ),
            None
        );
        assert_eq!(
            scissor(
                Rect {
                    w: f32::NAN,
                    ..rect
                },
                10,
                10
            ),
            None
        );
    }
}
