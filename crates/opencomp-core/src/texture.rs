//! The renderer's textures: an uploaded sRGB bitmap and the bind group that samples it.
//!
//! One pipeline serves both flat colours and bitmaps. Every draw binds a texture; a draw with no
//! image binds the built-in 1x1 white texture, so its colour multiplies a white texel and comes
//! out unchanged.
//!
//! Ids are chosen by the caller, not handed out by the registry: the compositor already names its
//! surfaces, and a caller that knows the id needs no map of its own. Zero is reserved for the
//! built-in white texture.

use std::collections::HashMap;

use crate::{TextureId, WHITE};

/// The textures an upload has made, and the layout every draw binds against.
pub(crate) struct Textures {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    white: wgpu::BindGroup,
    entries: HashMap<TextureId, wgpu::BindGroup>,
}

impl Textures {
    /// Create the layout, the sampler and the built-in white texture.
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("opencomp texture"),
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
        // Linear filtering: a composition surface is usually drawn scaled, and text and images
        // look better smoothed than pixelated.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("opencomp texture"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // 0xff decodes to 1.0 linear, so the fragment colour is the vertex colour untouched.
        let texture = Self::create(device, queue, 1, 1, &[255, 255, 255, 255]);
        let view = texture.create_view(&Default::default());
        let white = bind_group(device, &layout, &sampler, &view);
        Self {
            layout,
            sampler,
            white,
            entries: HashMap::new(),
        }
    }

    pub(crate) fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// Upload `pixels`, sRGB RGBA8 and row-major, under a caller-chosen id.
    ///
    /// Uploading an id again replaces the bitmap. An empty size, a slice that does not hold
    /// `width * height * 4` bytes, and the reserved id zero are all ignored.
    pub(crate) fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: TextureId,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) {
        let expected = width as usize * height as usize * 4;
        if id == WHITE || expected == 0 || pixels.len() != expected {
            return;
        }
        let texture = Self::create(device, queue, width, height, pixels);
        let view = texture.create_view(&Default::default());
        let group = bind_group(device, &self.layout, &self.sampler, &view);
        self.entries.insert(id, group);
    }

    /// Drop the bitmap for `id`; draws that still name it fall back to the white texture.
    pub(crate) fn release(&mut self, id: TextureId) {
        self.entries.remove(&id);
    }

    /// A 1-D sRGB ramp for `gradient`, plus the bind group that samples it.
    ///
    /// The ramp is built on the CPU ([`crate::Gradient::ramp`]) because a gradient shader that
    /// evaluated stops itself would need an array the pipeline cannot size. A 256-entry texture
    /// is exact enough for a preview and keeps one pipeline for flat, bitmap and gradient draws.
    pub(crate) fn ramp(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        gradient: &crate::Gradient,
    ) -> (wgpu::Texture, wgpu::BindGroup) {
        let pixels = gradient.ramp();
        let texture = Self::create(device, queue, crate::RAMP_ENTRIES as u32, 1, &pixels);
        let view = texture.create_view(&Default::default());
        let group = bind_group(device, &self.layout, &self.sampler, &view);
        (texture, group)
    }

    /// The bind group a draw naming `id` uses; an unknown id draws as a flat colour.
    pub(crate) fn bind_group(&self, id: TextureId) -> &wgpu::BindGroup {
        self.entries.get(&id).unwrap_or(&self.white)
    }

    fn create(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        pixels: &[u8],
    ) -> wgpu::Texture {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("opencomp texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        texture
    }
}

/// One bitmap and the shared sampler, as the shader sees them.
fn bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("opencomp texture"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}
