//! The same tree, in a window: `render_to_surface` against a real swapchain.
//!
//! The window and the surface belong to the host, which is the split a platform
//! integration needs: `opencomp` on Windows hands the HWND's surface to this,
//! and nothing in the renderer knows what a window is.
//!
//! `cargo run -p opencomp-core --example window`

use std::sync::Arc;

use opencomp_core::{Host, Visual, srgb};
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
    host: Host,
}

impl Frame {
    fn new(window: Arc<Window>) -> Option<Self> {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(
            wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(window.clone())),
        );
        let surface = instance.create_surface(window).ok()?;
        let background = [srgb(0.09), srgb(0.10), srgb(0.12), 1.0];
        let host = Host::new(
            &instance,
            surface,
            (size.width.max(1), size.height.max(1)),
            background,
        )?;
        Some(Self { host })
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
            frame.host.set_root(scene());
            frame.host.set_scale(window.scale_factor() as f32);
        }
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(frame) = &mut self.frame else { return };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => frame.host.resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                if !frame.host.present() {
                    return;
                }
                self.presented += 1;
                if self.limit.is_some_and(|limit| self.presented >= limit) {
                    let (width, height) = frame.host.size();
                    println!(
                        "presented {} frame(s) at {width}x{height} {:?}",
                        self.presented,
                        frame.host.format()
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
    let mut panel = Visual::rect(272.0, 192.0, [srgb(0.16), srgb(0.17), srgb(0.20), 1.0])
        .at(24.0, 24.0)
        .clip_to(0.0, 0.0, 272.0, 192.0);
    for (index, color) in [
        [srgb(0.29), srgb(0.62), srgb(0.96), 1.0],
        [srgb(0.35), srgb(0.85), srgb(0.55), 0.5],
        [srgb(0.96), srgb(0.45), srgb(0.35), 1.0],
    ]
    .into_iter()
    .enumerate()
    {
        panel.push(Visual::rect(240.0, 40.0, color).at(16.0, 16.0 + index as f32 * 56.0));
    }
    panel
}
