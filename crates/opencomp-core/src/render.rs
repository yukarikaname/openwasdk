//! The wgpu renderer: draws a [`Visual`] tree into a texture or a swapchain image.

use bytemuck::{Pod, Zeroable};

use crate::tree::{Visual, flatten};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    color: [f32; 4],
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
    size: (u32, u32),
    /// Device pixels per composition unit: the display scale factor.
    scale: f32,
    background: [f32; 4],
    root: Visual,
}

impl Renderer {
    /// A renderer that draws into textures, for tests and for a host that owns its own surface.
    pub fn headless(size: (u32, u32), background: [f32; 4]) -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .ok()?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
        Some(Self::new(device, queue, size, background))
    }

    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        size: (u32, u32),
        background: [f32; 4],
    ) -> Self {
        Self::with_format(
            device,
            queue,
            size,
            background,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        )
    }

    /// The same, for a host whose surface uses another format (Bgra8 on some compositors).
    pub fn with_format(
        device: wgpu::Device,
        queue: wgpu::Queue,
        size: (u32, u32),
        background: [f32; 4],
        format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("opencomp solid"),
            source: wgpu::ShaderSource::Wgsl(include_str!("solid.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("opencomp solid"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("opencomp solid"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("opencomp solid"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            device,
            queue,
            pipeline,
            layout,
            format,
            size,
            scale: 1.0,
            background,
            root: Visual::default(),
        }
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// The host resized its surface; the next frame draws at the new size.
    pub fn set_size(&mut self, size: (u32, u32)) {
        self.size = size;
    }

    /// The host's scale factor. Composition units are device independent; the surface is not.
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn root_mut(&mut self) -> &mut Visual {
        &mut self.root
    }

    pub fn set_root(&mut self, root: Visual) {
        self.root = root;
    }

    /// Render the tree into the swapchain image the host acquired.
    pub fn render_to_surface(&self, frame: &wgpu::SurfaceTexture) {
        let view = frame.texture.create_view(&Default::default());
        self.render(&view);
    }

    /// Render the tree into `target`, which must be `size()` and `format()`.
    pub fn render(&self, target: &wgpu::TextureView) {
        let mut draws = Vec::new();
        flatten(&self.root, (0.0, 0.0), 1.0, None, &mut draws);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("opencomp"),
            });
        {
            let [r, g, b, a] = self.background;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("opencomp"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: r as f64,
                            g: g as f64,
                            b: b as f64,
                            a: a as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            for draw in &draws {
                let (x, y, width, height) = draw.rect.as_pixels(self.scale);
                if width == 0 || height == 0 || x >= self.size.0 || y >= self.size.1 {
                    continue;
                }
                let uniforms = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("opencomp solid"),
                        contents: bytemuck::bytes_of(&Uniforms { color: draw.color }),
                        usage: wgpu::BufferUsages::UNIFORM,
                    });
                let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("opencomp solid"),
                    layout: &self.layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniforms.as_entire_binding(),
                    }],
                });
                pass.set_bind_group(0, &bind_group, &[]);
                pass.set_scissor_rect(
                    x,
                    y,
                    width.min(self.size.0 - x),
                    height.min(self.size.1 - y),
                );
                pass.draw(0..3, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
    }

    /// Render and read the pixels back, row-major RGBA8.
    pub fn render_to_pixels(&self) -> Vec<u8> {
        let (width, height) = self.size;
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("opencomp readback"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        self.render(&view);

        // A copy row has to be a multiple of 256 bytes.
        let unpadded = width * 4;
        let padded = unpadded.div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("opencomp readback"),
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        buffer.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        self.device.poll(wgpu::PollType::wait_indefinitely()).ok();
        let Ok(mapped) = buffer.slice(..).get_mapped_range() else {
            return Vec::new();
        };
        let mut pixels = Vec::with_capacity((unpadded * height) as usize);
        for row in 0..height {
            let start = (row * padded) as usize;
            pixels.extend_from_slice(&mapped[start..start + unpadded as usize]);
        }
        pixels
    }
}

use wgpu::util::DeviceExt as _;
