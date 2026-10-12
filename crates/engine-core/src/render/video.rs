use std::borrow::Cow;

use bytemuck::{Pod, Zeroable};
use deflorta_assets::video::{Plane, VideoFrame};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Conversion {
    rows: [[f32; 4]; 3],
    sampling: [u32; 4],
}

pub(super) struct Planes {
    textures: [wgpu::Texture; 4],
    uniform: wgpu::Buffer,
    binding: wgpu::BindGroup,
}

pub(super) struct Converter {
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}

impl Converter {
    pub fn new(device: &wgpu::Device) -> Self {
        let mut entries: Vec<_> = (0..4)
            .map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            })
            .collect();
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 4,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("video planes"),
            entries: &entries,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("video conversion"),
            source: wgpu::ShaderSource::SpirV(Cow::Borrowed(wgpu::include_spirv_source!(concat!(
                env!("OUT_DIR"),
                "/shaders/video.spv"
            )))),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("video conversion"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("video conversion"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self { layout, pipeline }
    }

    pub fn allocate(&self, device: &wgpu::Device, frame: &VideoFrame) -> Planes {
        let sizes = sizes(frame);
        let textures = sizes.map(|(width, height)| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("video plane"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            })
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("video conversion"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let views = textures
            .each_ref()
            .map(|t| t.create_view(&wgpu::TextureViewDescriptor::default()));
        let mut entries: Vec<_> = views
            .iter()
            .enumerate()
            .map(|(index, view)| wgpu::BindGroupEntry {
                binding: u32::try_from(index).expect("four planes"),
                resource: wgpu::BindingResource::TextureView(view),
            })
            .collect();
        entries.push(wgpu::BindGroupEntry {
            binding: 4,
            resource: uniform.as_entire_binding(),
        });
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("video planes"),
            layout: &self.layout,
            entries: &entries,
        });
        Planes {
            textures,
            uniform,
            binding,
        }
    }

    pub fn convert(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        output: &wgpu::Texture,
        planes: &Planes,
        frame: &VideoFrame,
    ) {
        for (texture, plane) in planes
            .textures
            .iter()
            .zip(frame.planes.iter().chain(frame.alpha.iter()))
        {
            upload(queue, texture, plane);
        }
        queue.write_buffer(
            &planes.uniform,
            0,
            bytemuck::bytes_of(&Conversion {
                rows: frame.rgb_transform(),
                sampling: [
                    u32::from(frame.subsampling.0),
                    u32::from(frame.subsampling.1),
                    u32::from(frame.alpha.is_some()),
                    0,
                ],
            }),
        );
        let view = output.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("video conversion"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("video conversion"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &planes.binding, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit(Some(encoder.finish()));
    }
}

impl Planes {
    pub fn matches(&self, frame: &VideoFrame) -> bool {
        self.textures
            .iter()
            .zip(sizes(frame))
            .all(|(texture, (width, height))| {
                texture.width() == width && texture.height() == height
            })
    }
}

fn sizes(frame: &VideoFrame) -> [(u32, u32); 4] {
    [
        (frame.planes[0].width, frame.planes[0].height),
        (frame.planes[1].width, frame.planes[1].height),
        (frame.planes[2].width, frame.planes[2].height),
        frame.alpha.as_ref().map_or((1, 1), |a| (a.width, a.height)),
    ]
}

fn upload(queue: &wgpu::Queue, texture: &wgpu::Texture, plane: &Plane) {
    queue.write_texture(
        texture.as_image_copy(),
        &plane.data,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(plane.stride),
            rows_per_image: None,
        },
        texture.size(),
    );
}
