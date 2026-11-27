//! The frame loop: a window's presenter, and the thread that drives it.
//!
//! A composition-only client hands over an `HWND` as a `WindowId` and expects frames;
//! nobody runs a message loop on our behalf. So the bridge starts a thread that
//! snapshots the island's tree and draws it at about sixty hertz. There is no commit
//! to wait for, which is how the lifted compositor behaves.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use windows_numerics::Vector2;

use crate::d2d::Presenter;
use crate::node::{self, NodeRef};

/// The frame interval. A slower tree only costs a snapshot per tick.
const INTERVAL: Duration = Duration::from_millis(16);

/// Where the window is and whether frames are still wanted.
struct Shared {
    size: Mutex<(u32, u32)>,
    stop: AtomicBool,
}

impl Shared {
    fn size(&self) -> (u32, u32) {
        *self.size.lock().unwrap()
    }

    fn set_size(&self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            *self.size.lock().unwrap() = (width, height);
        }
    }

    fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

/// A window that presents an island's tree until it is stopped.
pub struct Window {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Window {
    /// Start presenting `root` on `hwnd`, which the caller keeps alive.
    ///
    /// Returns `None` when no device can be created for the window; the bridge then
    /// reports nothing rather than pretending a frame will appear.
    pub fn start(root: NodeRef, hwnd: u64, size: (u32, u32)) -> Option<Self> {
        let presenter = Presenter::new(hwnd, size.0, size.1)?;
        let shared = Arc::new(Shared {
            size: Mutex::new(size),
            stop: AtomicBool::new(false),
        });
        let thread = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("opencomp-present".into())
                .spawn(move || frame_loop(presenter, root, shared))
                .ok()?
        };
        Some(Self {
            shared,
            thread: Some(thread),
        })
    }

    pub fn size(&self) -> (u32, u32) {
        self.shared.size()
    }

    pub fn resize(&self, width: u32, height: u32) {
        self.shared.set_size(width, height);
    }

    /// Stop presenting and wait for the thread.
    pub fn stop(&mut self) {
        self.shared.stop();
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

fn frame_loop(mut presenter: Presenter, root: NodeRef, shared: Arc<Shared>) {
    let mut current = presenter.size();
    while !shared.stopped() {
        let (width, height) = shared.size();
        if (width, height) != current {
            presenter.resize(width, height);
            current = presenter.size();
        }
        let parent = Vector2 {
            x: current.0 as f32,
            y: current.1 as f32,
        };
        if let Some(tree) = node::snapshot_in(&root, parent) {
            let draws = tree.draws();
            upload_surfaces(&mut presenter, &root);
            presenter.present(&draws);
        }
        std::thread::sleep(INTERVAL);
    }
}

/// Push every surface in the tree into the presenter.
///
/// A surface has no change counter yet, so the pixels go over on every frame. That is
/// correct and wasteful; a generation counter on the surface is the next step.
fn upload_surfaces(presenter: &mut Presenter, root: &NodeRef) {
    if let Some((id, width, height, pixels)) = crate::surface::surface_image(root) {
        presenter.upload(id, width, height, &pixels);
    }
    let children = root.lock().unwrap().children.clone();
    for child in &children {
        upload_surfaces(presenter, child);
    }
}
