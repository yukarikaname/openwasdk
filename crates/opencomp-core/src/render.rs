//! The wgpu renderer: draws a [`Visual`] tree into a texture or a swapchain image.
//!
//! Each draw is one quadrilateral, six vertices in a vertex buffer built for the frame. The colour
//! and the texture coordinates ride along with the vertices, so there is no uniform to allocate.
//! Every draw binds a texture: an image from the registry, or the built-in white one that leaves a
//! flat colour alone. The CPU applies the tree's affine transform and then the device scale, so
//! the shader only forwards clip-space corners. A scissor stays the only clip mechanism; see
//! [`crate::tree`] for what that means when a rotation is involved.
//!
//! Vertices also carry their place in the node's own coordinates, the node's half size and its
//! four corner radii. The fragment stage rebuilds the local position from them and rounds the
//! outline there, so a radius follows the transform instead of the screen.

use bytemuck::{Pod, Zeroable};

use crate::texture::Textures;
use crate::tree::{Draw, Rect, Visual, flatten_root};
use crate::{TextureId, WHITE};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    /// Clip-space position; the affine transform and the device scale are already applied.
    position: [f32; 2],
    color: [f32; 4],
    uv: [f32; 2],
    /// The same corner in the node's own coordinates; the fragment stage interpolates it, so a
    /// fragment knows where it sits before the affine transform and the device scale.
    local: [f32; 2],
    /// Half the node's size, the same on every vertex of the quad.
    half_size: [f32; 2],
    /// The four corner radii, the same on every vertex of the quad.
    radius: [f32; 4],
    /// -1 for a flat paint, 0 for a linear ramp, 1 for a radial one.
    gradient_kind: f32,
    /// The ramp's start or centre, in the node's own units.
    gradient_a: [f32; 2],
    /// The ramp's end or radius, in the node's own units.
    gradient_b: [f32; 2],
    /// -1 for a rectangle or rounded rectangle, 0 for an ellipse, 1 for a line.
    shape_kind: f32,
    /// The ellipse centre or the line's start, in the node's own units.
    shape_a: [f32; 2],
    /// The ellipse radii or the line's end, in the node's own units.
    shape_b: [f32; 2],
    /// The line's stroke width, in the node's own units; zero elsewhere.
    shape_width: f32,
}

const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 13] = wgpu::vertex_attr_array![
    0 => Float32x2,
    1 => Float32x4,
    2 => Float32x2,
    3 => Float32x2,
    4 => Float32x2,
    5 => Float32x4,
    6 => Float32,
    7 => Float32x2,
    8 => Float32x2,
    9 => Float32,
    10 => Float32x2,
    11 => Float32x2,
    12 => Float32
];

const VERTEX_LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
    array_stride: core::mem::size_of::<Vertex>() as wgpu::BufferAddress,
    step_mode: wgpu::VertexStepMode::Vertex,
    attributes: &VERTEX_ATTRIBUTES,
};

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    format: wgpu::TextureFormat,
    size: (u32, u32),
    /// Device pixels per composition unit: the display scale factor.
    scale: f32,
    background: [f32; 4],
    textures: Textures,
    root: Visual,
    /// The next id handed to a rasterised string. High, so a client's own ids do not collide.
    next_text: TextureId,
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
        // Flat colours and bitmaps share one pipeline: a draw without an image binds the white
        // texture, so the only difference between the two cases is which bind group is set.
        let textures = Textures::new(&device, &queue);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("opencomp solid"),
            bind_group_layouts: &[Some(textures.layout())],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("opencomp solid"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(VERTEX_LAYOUT)],
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
            format,
            size,
            scale: 1.0,
            background,
            textures,
            root: Visual::default(),
            next_text: 0x1000_0000,
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

    /// Give the renderer the tree to draw.
    ///
    /// Text is rasterised here rather than in the draw loop: the glyphs become a texture
    /// like any bitmap the client uploaded, so flattening and drawing stay unchanged.
    pub fn set_root(&mut self, mut root: Visual) {
        self.paint_text(&mut root);
        self.root = root;
    }

    fn paint_text(&mut self, visual: &mut Visual) {
        if let Some(text) = visual.text.take()
            && !text.text.trim().is_empty()
            && let Some(raster) = crate::text::rasterize(&text.text, text.size, text.color)
        {
            let id = self.next_text;
            self.next_text = self.next_text.wrapping_add(1);
            self.upload_rgba(id, raster.width, raster.height, &raster.pixels);
            visual.image = Some(id);
            visual.size = [raster.width as f32, raster.height as f32];
            // The raster already carries the colour; the quad multiplies by white.
            visual.color = [1.0, 1.0, 1.0, 1.0];
            visual.uv = [0.0, 0.0, 1.0, 1.0];
        }
        for child in &mut visual.children {
            self.paint_text(child);
        }
    }

    /// Upload a bitmap under a caller-chosen [`TextureId`], sRGB RGBA8 and row-major.
    ///
    /// Uploading an id again replaces the bitmap. An empty size, a slice that does not hold
    /// `width * height * 4` bytes, and the reserved id zero are all ignored.
    pub fn upload_rgba(&mut self, id: TextureId, width: u32, height: u32, pixels: &[u8]) {
        self.textures
            .upload(&self.device, &self.queue, id, width, height, pixels);
    }

    /// Drop a bitmap; visuals that still name its id draw as flat colours.
    pub fn release(&mut self, id: TextureId) {
        self.textures.release(id);
    }

    /// Render the tree into the swapchain image the host acquired.
    pub fn render_to_surface(&self, frame: &wgpu::SurfaceTexture) {
        let view = frame.texture.create_view(&Default::default());
        self.render(&view);
    }

    /// Render the tree into `target`, which must be `size()` and `format()`.
    pub fn render(&self, target: &wgpu::TextureView) {
        let mut draws = Vec::new();
        flatten_root(&self.root, &mut draws);

        let mut vertices = Vec::new();
        let mut scissors = Vec::new();
        let mut groups = Vec::new();
        // The bind group keeps the ramp alive, but the texture handle has to outlive the pass.
        let mut ramps = Vec::new();
        for draw in &draws {
            let Some(scissor) = scissor_rect(draw.scissor, self.scale, self.size) else {
                continue;
            };
            // A gradient with no stops asks for a picture that is not there; leave the pixel
            // alone rather than falling back to the flat colour the client replaced.
            if draw.gradient.as_ref().is_some_and(|g| !g.is_drawable()) {
                continue;
            }
            vertices.extend(quad(draw, self.scale, self.size));
            scissors.push(scissor);
            let group = match draw.gradient.as_ref() {
                Some(gradient) => {
                    let (texture, group) = self.textures.ramp(&self.device, &self.queue, gradient);
                    ramps.push(texture);
                    group
                }
                None => self
                    .textures
                    .bind_group(draw.image.unwrap_or(WHITE))
                    .clone(),
            };
            groups.push(group);
        }

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
            if !vertices.is_empty() {
                let buffer = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("opencomp solid"),
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                pass.set_pipeline(&self.pipeline);
                pass.set_vertex_buffer(0, buffer.slice(..));
                for (index, &(x, y, width, height)) in scissors.iter().enumerate() {
                    pass.set_bind_group(0, &groups[index], &[]);
                    pass.set_scissor_rect(x, y, width, height);
                    let first = (index * 6) as u32;
                    pass.draw(first..first + 6, 0..1);
                }
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

/// The six vertices of one draw: two triangles that cover the transformed rectangle.
fn quad(draw: &Draw, scale: f32, size: (u32, u32)) -> [Vertex; 6] {
    let width = size.0 as f32;
    let height = size.1 as f32;
    // The device scale is applied here, after the affine transform that produced the corners.
    let to_clip = |[x, y]: [f32; 2]| {
        [
            x * scale / width * 2.0 - 1.0,
            1.0 - y * scale / height * 2.0,
        ]
    };
    // Clockwise from the top-left, the same order as the corners.
    let [u0, v0, u1, v1] = draw.uv;
    let uvs = [[u0, v0], [u1, v0], [u1, v1], [u0, v1]];
    // The same corners in the node's own units, together with the half size the shader
    // measures the rounded outline against.
    let [width, height] = draw.size;
    let locals = [[0.0, 0.0], [width, 0.0], [width, height], [0.0, height]];
    let half_size = [width * 0.5, height * 0.5];
    // A gradient replaces the flat colour's rgb; its alpha stays the opacity that multiplies
    // the ramp's own alpha, exactly as the D2D brush opacity does.
    let color = if draw.gradient.is_some() {
        [1.0, 1.0, 1.0, draw.color[3]]
    } else {
        draw.color
    };
    let (gradient_kind, gradient_a, gradient_b) = match draw.gradient.as_ref() {
        Some(gradient) => match gradient.shape {
            crate::GradientShape::Linear { start, end } => (0.0, start, end),
            crate::GradientShape::Radial { center, radius } => (1.0, center, radius),
        },
        None => (-1.0, [0.0; 2], [0.0; 2]),
    };
    // A shape replaces the rectangle's corner rounding with its own outline; the quad still
    // covers the node's rectangle and the fragment stage discards the pixels outside.
    let (shape_kind, shape_a, shape_b, shape_width) = match draw.shape {
        Some(crate::Shape::Ellipse { center, radius }) => (0.0, center, radius, 0.0),
        Some(crate::Shape::Line { start, end, width }) => (1.0, start, end, width),
        None => (-1.0, [0.0; 2], [0.0; 2], 0.0),
    };
    let at = |index: usize| Vertex {
        position: to_clip(draw.corners[index]),
        color,
        uv: uvs[index],
        local: locals[index],
        half_size,
        radius: draw.radius,
        gradient_kind,
        gradient_a,
        gradient_b,
        shape_kind,
        shape_a,
        shape_b,
        shape_width,
    };
    [at(0), at(1), at(2), at(0), at(2), at(3)]
}

/// The device-pixel scissor for `bounds`, or `None` when the rectangle misses the target.
fn scissor_rect(bounds: Rect, scale: f32, size: (u32, u32)) -> Option<(u32, u32, u32, u32)> {
    let width = size.0 as f32;
    let height = size.1 as f32;
    // Round outwards: a scissor may pass extra pixels but must never cut into the quad.
    let left = (bounds.x * scale).floor().clamp(0.0, width);
    let top = (bounds.y * scale).floor().clamp(0.0, height);
    let right = ((bounds.x + bounds.width) * scale).ceil().clamp(0.0, width);
    let bottom = ((bounds.y + bounds.height) * scale)
        .ceil()
        .clamp(0.0, height);
    (right > left && bottom > top).then_some((
        left as u32,
        top as u32,
        (right - left) as u32,
        (bottom - top) as u32,
    ))
}

use wgpu::util::DeviceExt as _;
