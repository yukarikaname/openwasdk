//! A host surface: the swapchain, the device and the renderer, in one place.
//!
//! `Renderer` draws into whatever texture it is handed; `Host` is the part that owns
//! a window's surface, keeps the configuration in step with the window's size and
//! presents. A client (the examples) and the Windows DLL both use this, which is why
//! it takes a surface rather than a window: how the handle was obtained is not this
//! module's business.

use crate::{Renderer, Visual};

/// A window that can be drawn into, as long as the caller keeps its handle alive.
pub struct Host {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
}

impl Host {
    /// Wrap a surface the caller created and configured nothing on.
    ///
    /// Returns `None` when the machine has no adapter that can drive this surface.
    pub fn new(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        size: (u32, u32),
        background: [f32; 4],
    ) -> Option<Self> {
        let size = (size.0.max(1), size.1.max(1));
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .ok()?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb)
            .unwrap_or(capabilities.formats[0]);
        let mut config = surface.get_default_config(&adapter, size.0, size.1)?;
        config.format = format;
        surface.configure(&device, &config);
        let renderer =
            Renderer::with_format(device.clone(), queue.clone(), size, background, format);
        Some(Self {
            surface,
            device,
            queue,
            config,
            renderer,
        })
    }

    pub fn renderer_mut(&mut self) -> &mut Renderer {
        &mut self.renderer
    }

    pub fn set_root(&mut self, root: Visual) {
        self.renderer.set_root(root);
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.renderer.set_scale(scale);
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Follow the window's new size. Zero-sized windows are ignored: a minimised
    /// window has no swapchain to configure and asking for one is a validation error.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.renderer.set_size((width, height));
    }

    /// Draw the current tree into the next frame and present it.
    ///
    /// Returns false when the surface did not hand out a frame (occluded, outdated);
    /// the caller should simply try again on the next tick.
    pub fn present(&mut self) -> bool {
        use wgpu::CurrentSurfaceTexture as Acquired;
        let frame = match self.surface.get_current_texture() {
            Acquired::Success(frame) | Acquired::Suboptimal(frame) => frame,
            Acquired::Outdated | Acquired::Lost => {
                self.surface.configure(&self.device, &self.config);
                return false;
            }
            _ => return false,
        };
        self.renderer.render_to_surface(&frame);
        self.queue.present(frame);
        true
    }
}
