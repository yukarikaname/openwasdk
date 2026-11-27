//! The same tree, in a window: `render_to_surface` against a real swapchain.
//!
//! The window and the surface belong to the host, which is the split a platform
//! integration needs: `opencomp` on Windows hands the HWND's surface to this,
//! and nothing in the renderer knows what a window is.
//!
//! `cargo run -p opencomp-core --example window`

use std::sync::Arc;

use opencomp_core::{Renderer, Visual};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    // `OPENCOMP_FRAMES` makes the example exit after that many presented frames, which is how
    // it can be run without a person watching the window.
    let limit = std::env::var("OPENCOMP_FRAMES")
        .ok()
        .and_then(|value| value.parse().ok());
    event_loop.run_app(&mut App {
        limit,
        ..App::default()
    })?;
    Ok(())
}

#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    frame: Option<Frame>,
    presented: u32,
    limit: Option<u32>,
}

struct Frame {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
}

impl Frame {
    fn new(window: Arc<Window>) -> Option<Self> {
        let size = window.inner_size();
        let (width, height) = (size.width.max(1), size.height.max(1));
        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(window.clone())),
        );
        let surface = instance.create_surface(window).ok()?;
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
        let mut config = surface.get_default_config(&adapter, width, height)?;
        config.format = format;
        surface.configure(&device, &config);
        let renderer = Renderer::with_format(
            device.clone(),
            queue.clone(),
            (width, height),
            [0.05, 0.06, 0.08, 1.0],
            format,
        );
        Some(Self {
            device,
            queue,
            surface,
            config,
            renderer,
        })
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.renderer.set_size((width, height));
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes().with_title("opencomp"))
                .expect("window"),
        );
        self.frame = Frame::new(window.clone());
        if let Some(frame) = &mut self.frame {
            frame.renderer.set_root(scene());
        }
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(frame) = &mut self.frame else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => frame.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                use wgpu::CurrentSurfaceTexture as Acquired;
                let texture = match frame.surface.get_current_texture() {
                    Acquired::Success(texture) | Acquired::Suboptimal(texture) => texture,
                    // Outdated and Lost are handled by the resize arm; the rest skip the frame.
                    _ => return,
                };
                frame.renderer.render_to_surface(&texture);
                frame.queue.present(texture);
                self.presented += 1;
                if self.limit.is_some_and(|limit| self.presented >= limit) {
                    let (width, height) = frame.renderer.size();
                    println!(
                        "presented {} frame(s) at {width}x{height} {:?}",
                        self.presented,
                        frame.renderer.format()
                    );
                    event_loop.exit();
                    return;
                }
            }
            _ => return,
        }
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

/// The same panel the offscreen example draws.
fn scene() -> Visual {
    let mut panel = Visual::rect(272.0, 192.0, [0.10, 0.11, 0.14, 1.0])
        .at(24.0, 24.0)
        .clip_to(0.0, 0.0, 272.0, 192.0);
    for (index, color) in [
        [0.29, 0.62, 0.96, 1.0],
        [0.35, 0.85, 0.55, 0.5],
        [0.96, 0.45, 0.35, 1.0],
    ]
    .into_iter()
    .enumerate()
    {
        panel.push(Visual::rect(240.0, 40.0, color).at(16.0, 16.0 + index as f32 * 56.0));
    }
    panel
}
