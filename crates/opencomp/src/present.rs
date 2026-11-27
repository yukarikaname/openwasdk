//! The frame loop: a window's presenter, and the thread that drives it.
//!
//! A composition-only client hands over an `HWND` as a `WindowId` and expects frames;
//! nobody runs a message loop on our behalf. So the bridge starts a thread that
//! snapshots the island's tree and draws it at about sixty hertz. There is no commit
//! to wait for, which is how the lifted compositor behaves.
//!
//! The device and the swapchain are made **inside** that thread: under Wine they can
//! take seconds on a software backend, and `Show` is called on a client's UI thread,
//! which must not inherit that delay.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use windows_numerics::Vector2;

use crate::d2d::Presenter;
use crate::node::{self, NodeRef};

/// The frame interval. A slower tree only costs a snapshot per tick.
const INTERVAL: Duration = Duration::from_millis(16);

/// How long `stop` waits for the frame loop before leaving the thread to the process
/// exit. A present that never returns must not keep a client alive.
const STOP_TIMEOUT: Duration = Duration::from_secs(2);

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

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

/// A window that presents an island's tree until it is stopped.
pub struct Window {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
    /// Signalled when the frame loop returns.
    finished: Receiver<()>,
}

impl Window {
    /// Present `root` on `hwnd`, which the caller keeps alive.
    pub fn start(root: NodeRef, hwnd: u64, size: (u32, u32)) -> Option<Self> {
        let shared = Arc::new(Shared {
            size: Mutex::new(size),
            stop: AtomicBool::new(false),
        });
        let (finished, done) = mpsc::channel();
        let thread = {
            let shared = shared.clone();
            std::thread::Builder::new()
                .name("opencomp-present".into())
                .spawn(move || {
                    frame_loop(root, hwnd, shared);
                    let _ = finished.send(());
                })
                .ok()?
        };
        Some(Self {
            shared,
            thread: Some(thread),
            finished: done,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        self.shared.size()
    }

    pub fn resize(&self, width: u32, height: u32) {
        self.shared.set_size(width, height);
    }

    /// Stop presenting. Waits for the loop, but not forever.
    pub fn stop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            match self.finished.recv_timeout(STOP_TIMEOUT) {
                Ok(()) | Err(RecvTimeoutError::Disconnected) => {
                    let _ = thread.join();
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.stop();
    }
}

fn frame_loop(root: NodeRef, hwnd: u64, shared: Arc<Shared>) {
    let (width, height) = shared.size();
    eprintln!("opencomp: frame thread started for a {width}x{height} window");
    let Some(mut presenter) = Presenter::new(hwnd, width, height) else {
        eprintln!("opencomp: no device for this window; nothing will be presented");
        return;
    };
    eprintln!("opencomp: presenter ready");
    let mut current = presenter.size();
    let mut reported = false;
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
            let presented = presenter.present(&draws);
            if presented && !reported {
                reported = true;
                eprintln!("opencomp: first frame presented");
            }
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
