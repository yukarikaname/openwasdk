//! The portable half of the compositor: a visual tree and a wgpu renderer.
//!
//! `opencomp` is the Windows ABI shell around this; the core has no WinRT in it, so it builds and
//! tests on every host. That is what makes a macOS backend possible: the renderer talks to Metal
//! through wgpu the same way it talks to Vulkan or D3D12 elsewhere.

use bytemuck::{Pod, Zeroable};

/// A solid rectangle in canvas pixels: origin, then size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn as_pixels(&self, scale: f32) -> (u32, u32, u32, u32) {
        let x = (self.x * scale).round().max(0.0) as u32;
        let y = (self.y * scale).round().max(0.0) as u32;
        let width = (self.width * scale).round().max(0.0) as u32;
        let height = (self.height * scale).round().max(0.0) as u32;
        (x, y, width, height)
    }
}

/// A coloured rectangle. Brushes, clips and animations attach here as they land.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Visual {
    pub rect: Rect,
    pub color: [f32; 4],
}

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
    background: [f32; 4],
    visuals: Vec<Visual>,
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
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
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
        Self {
            device,
            queue,
            pipeline,
            layout,
            format,
            size,
            background,
            visuals: Vec::new(),
        }
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn visuals_mut(&mut self) -> &mut Vec<Visual> {
        &mut self.visuals
    }

    /// Render the tree into `target`, which must be `size()` and `format()`.
    pub fn render(&self, target: &wgpu::TextureView) {
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
            for visual in &self.visuals {
                let (x, y, width, height) = visual.rect.as_pixels(1.0);
                if width == 0 || height == 0 || x >= self.size.0 || y >= self.size.1 {
                    continue;
                }
                let uniforms = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("opencomp solid"),
                        contents: bytemuck::bytes_of(&Uniforms {
                            color: visual.color,
                        }),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * width + x) * 4) as usize;
        pixels[at..at + 4].try_into().unwrap()
    }

    #[test]
    fn a_visual_draws_over_the_background() {
        let Some(mut renderer) = Renderer::headless((16, 16), [0.0, 0.0, 1.0, 1.0]) else {
            // No adapter on this machine; the test cannot say anything either way.
            return;
        };
        renderer.visuals_mut().push(Visual {
            rect: Rect::new(4.0, 4.0, 8.0, 8.0),
            color: [1.0, 0.0, 0.0, 1.0],
        });
        let pixels = renderer.render_to_pixels();
        assert_eq!(pixel(&pixels, 16, 8, 8), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 16, 0, 0), [0, 0, 255, 255]);
    }

    #[test]
    fn a_visual_outside_the_target_is_skipped() {
        let Some(mut renderer) = Renderer::headless((8, 8), [0.0, 0.0, 0.0, 1.0]) else {
            return;
        };
        renderer.visuals_mut().push(Visual {
            rect: Rect::new(20.0, 20.0, 4.0, 4.0),
            color: [1.0, 1.0, 1.0, 1.0],
        });
        let pixels = renderer.render_to_pixels();
        assert_eq!(pixel(&pixels, 8, 0, 0), [0, 0, 0, 255]);
    }
}
