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

/// One node of the tree. The shape is deliberately the WinRT one: an offset, a size, a colour and
/// children, so `opencomp` can hand `ContainerVisual` and `SpriteVisual` onto it one to one.
#[derive(Clone, Debug, PartialEq)]
pub struct Visual {
    /// Translation from the parent's origin.
    pub offset: [f32; 2],
    pub size: [f32; 2],
    /// Straight (unpremultiplied) colour.
    pub color: [f32; 4],
    /// Multiplied down the tree, like a group's opacity.
    pub opacity: f32,
    /// Children are clipped to this rectangle in the node's own coordinates.
    pub clip: Option<(f32, f32, f32, f32)>,
    pub children: Vec<Visual>,
}

impl Default for Visual {
    fn default() -> Self {
        Self {
            offset: [0.0, 0.0],
            size: [0.0, 0.0],
            color: [0.0, 0.0, 0.0, 0.0],
            opacity: 1.0,
            clip: None,
            children: Vec::new(),
        }
    }
}

impl Visual {
    /// A coloured rectangle at the parent's origin.
    pub fn rect(width: f32, height: f32, color: [f32; 4]) -> Self {
        Self {
            size: [width, height],
            color,
            ..Self::default()
        }
    }

    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.offset = [x, y];
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Clip children to a rectangle in this node's coordinates.
    pub fn clip_to(mut self, x: f32, y: f32, width: f32, height: f32) -> Self {
        self.clip = Some((x, y, width, height));
        self
    }

    pub fn push(&mut self, child: Visual) -> &mut Self {
        self.children.push(child);
        self
    }
}

/// The part of `rect` that survives `clip`, or `None` when nothing does.
fn intersect(rect: Rect, clip: (f32, f32, f32, f32)) -> Option<Rect> {
    let left = rect.x.max(clip.0);
    let top = rect.y.max(clip.1);
    let right = (rect.x + rect.width).min(clip.0 + clip.2);
    let bottom = (rect.y + rect.height).min(clip.1 + clip.3);
    (right > left && bottom > top).then(|| Rect::new(left, top, right - left, bottom - top))
}

/// A rectangle ready to draw, with the tree's translations and opacities applied.
#[derive(Clone, Copy, Debug)]
struct Draw {
    rect: Rect,
    color: [f32; 4],
}

/// Flatten the tree into draw order, carrying the accumulated clip rectangle in canvas pixels.
fn flatten(
    visual: &Visual,
    origin: (f32, f32),
    opacity: f32,
    clip: Option<(f32, f32, f32, f32)>,
    out: &mut Vec<Draw>,
) {
    let x = origin.0 + visual.offset[0];
    let y = origin.1 + visual.offset[1];
    let alpha = opacity * visual.opacity;
    let draw = Rect::new(x, y, visual.size[0], visual.size[1]);
    if visual.size[0] > 0.0 && visual.size[1] > 0.0 && alpha > 0.0 && visual.color[3] > 0.0 {
        if let Some(clip) = clip {
            if let Some(visible) = intersect(draw, clip) {
                out.push(Draw {
                    rect: visible,
                    color: [
                        visual.color[0],
                        visual.color[1],
                        visual.color[2],
                        visual.color[3] * alpha,
                    ],
                });
            }
        } else {
            out.push(Draw {
                rect: draw,
                color: [
                    visual.color[0],
                    visual.color[1],
                    visual.color[2],
                    visual.color[3] * alpha,
                ],
            });
        }
    }
    let clip = visual.clip.map_or(clip, |local| {
        let absolute = (x + local.0, y + local.1, local.2, local.3);
        Some(match clip {
            Some(parent) => {
                let left = parent.0.max(absolute.0);
                let top = parent.1.max(absolute.1);
                let right = (parent.0 + parent.2).min(absolute.0 + absolute.2);
                let bottom = (parent.1 + parent.3).min(absolute.1 + absolute.3);
                (left, top, (right - left).max(0.0), (bottom - top).max(0.0))
            }
            None => absolute,
        })
    });
    for child in &visual.children {
        flatten(child, (x, y), alpha, clip, out);
    }
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
            background,
            root: Visual::default(),
        }
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
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
                let (x, y, width, height) = draw.rect.as_pixels(1.0);
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
        renderer.set_root(Visual::rect(8.0, 8.0, [1.0, 0.0, 0.0, 1.0]).at(4.0, 4.0));
        let pixels = renderer.render_to_pixels();
        assert_eq!(pixel(&pixels, 16, 8, 8), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 16, 0, 0), [0, 0, 255, 255]);
    }

    #[test]
    fn a_child_is_placed_relative_to_its_parent() {
        let Some(mut renderer) = Renderer::headless((16, 16), [0.0, 0.0, 0.0, 1.0]) else {
            return;
        };
        let mut parent = Visual::rect(12.0, 12.0, [0.0, 0.0, 0.0, 0.0]).at(2.0, 2.0);
        parent.push(Visual::rect(4.0, 4.0, [0.0, 1.0, 0.0, 1.0]).at(2.0, 2.0));
        renderer.set_root(parent);
        let pixels = renderer.render_to_pixels();
        assert_eq!(pixel(&pixels, 16, 5, 5), [0, 255, 0, 255]);
        assert_eq!(pixel(&pixels, 16, 2, 2), [0, 0, 0, 255]);
    }

    #[test]
    fn children_are_clipped_to_the_parent() {
        let Some(mut renderer) = Renderer::headless((16, 16), [0.0, 0.0, 1.0, 1.0]) else {
            return;
        };
        let mut parent = Visual::rect(4.0, 4.0, [0.0, 0.0, 0.0, 0.0])
            .at(4.0, 4.0)
            .clip_to(0.0, 0.0, 4.0, 4.0);
        parent.push(Visual::rect(8.0, 8.0, [1.0, 0.0, 0.0, 1.0]));
        renderer.set_root(parent);
        let pixels = renderer.render_to_pixels();
        assert_eq!(pixel(&pixels, 16, 5, 5), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 16, 10, 10), [0, 0, 255, 255]);
    }

    #[test]
    fn a_zero_opacity_visual_is_skipped() {
        let Some(mut renderer) = Renderer::headless((8, 8), [0.0, 0.0, 1.0, 1.0]) else {
            return;
        };
        renderer.set_root(
            Visual::rect(8.0, 8.0, [1.0, 0.0, 0.0, 1.0])
                .at(0.0, 0.0)
                .with_opacity(0.0),
        );
        let pixels = renderer.render_to_pixels();
        assert_eq!(pixel(&pixels, 8, 4, 4), [0, 0, 255, 255]);
    }

    #[test]
    fn a_visual_outside_the_target_is_skipped() {
        let Some(mut renderer) = Renderer::headless((8, 8), [0.0, 0.0, 0.0, 1.0]) else {
            return;
        };
        renderer.set_root(Visual::rect(4.0, 4.0, [1.0, 1.0, 1.0, 1.0]).at(20.0, 20.0));
        let pixels = renderer.render_to_pixels();
        assert_eq!(pixel(&pixels, 8, 0, 0), [0, 0, 0, 255]);
    }
}
