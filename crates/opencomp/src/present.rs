//! Presentation: the window the bridge was given, and the timer that draws into it.
//!
//! A composition-only client hands over an `HWND` as a `WindowId` and expects frames.
//! Nobody in that arrangement owns a message loop for us, so this module owns one
//! thing: a surface on that handle, plus a thread that presents the island's tree at
//! about sixty hertz. When the tree changes the next tick shows it; there is no
//! commit to wait for, which matches how the lifted compositor behaves.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use opencomp_core::Host;

use crate::node::{self, NodeRef};

/// The frame interval. Sixty hertz is what the display will show; a slower tree only
/// costs a snapshot per tick, which is the cheap half of a frame.
const INTERVAL: Duration = Duration::from_millis(16);

/// A window that presents an island's tree until it is stopped.
pub struct Window {
    root: NodeRef,
    size: Arc<Mutex<(u32, u32)>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Window {
    /// Start presenting `root` on `hwnd`, which the caller keeps alive.
    ///
    /// Returns `None` when no adapter can drive the surface; the bridge then reports a
    /// failure to create rather than pretending a frame will appear.
    pub fn start(root: NodeRef, hwnd: u64, size: (u32, u32)) -> Option<Self> {
        let host = Host::for_win32(hwnd, size, [0.0, 0.0, 0.0, 1.0])?;
        let stop = Arc::new(AtomicBool::new(false));
        let size = Arc::new(Mutex::new(size));
        let thread = {
            let root = root.clone();
            let stop = stop.clone();
            let size = size.clone();
            std::thread::Builder::new()
                .name("opencomp-present".into())
                .spawn(move || present_loop(host, root, size, stop))
                .ok()?
        };
        Some(Self {
            root,
            size,
            stop,
            thread: Some(thread),
        })
    }

    /// The size the caller last asked for, in composition units.
    pub fn size(&self) -> (u32, u32) {
        *self.size.lock().unwrap()
    }

    /// Follow a resize. The thread picks it up on the next tick.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            *self.size.lock().unwrap() = (width, height);
        }
    }

    /// Stop presenting and wait for the thread. Called from `Drop` and by the bridge.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.stop();
    }
}

fn present_loop(host: Host, root: NodeRef, size: Arc<Mutex<(u32, u32)>>, stop: Arc<AtomicBool>) {
    let mut host = host;
    let mut window_size = host.size();
    while !stop.load(Ordering::Relaxed) {
        let (width, height) = *size.lock().unwrap();
        if (width, height) != window_size {
            host.resize(width, height);
            window_size = (width, height);
        }
        let parent = windows_numerics::Vector2 {
            x: width as f32,
            y: height as f32,
        };
        let visual = node::snapshot_in(&root, parent);
        if let Some(visual) = visual {
            host.set_root(visual);
            host.present();
        }
        std::thread::sleep(INTERVAL);
    }
}
