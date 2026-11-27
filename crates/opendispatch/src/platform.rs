//! The Win32 half: one hidden window per queue, its message loop, and the
//! private messages that carry work and quit requests.
//!
//! The window is message-only, so it never appears; what it gives the queue is a
//! thread message queue and a window procedure that `DispatchMessage` reaches
//! from whatever loop the thread runs, ours or the engine's. `WM_APP + 1` drains
//! the task queue and `WM_APP + 2` turns into `WM_QUIT`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetMessageW,
    GetWindowLongPtrW, HWND_MESSAGE, KillTimer, MSG, PostMessageW, PostQuitMessage,
    RegisterClassExW, SetTimer, SetWindowLongPtrW, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_APP, WM_TIMER, WNDCLASSEXW,
};
use windows::core::PCWSTR;

use crate::queue::{Poster, Queue};
use crate::timer::TimerSlot;

const WM_DISPATCH: u32 = WM_APP + 1;
const WM_QUIT_LOCAL: u32 = WM_APP + 2;

/// `opendispatch` in UTF-16, with the terminator `RegisterClassExW` wants.
const CLASS_NAME: [u16; 13] = [
    0x006f, 0x0070, 0x0065, 0x006e, 0x0064, 0x0069, 0x0073, 0x0070, 0x0061, 0x0074, 0x0063, 0x0068,
    0x0000,
];

/// State the window procedure reaches from `GWLP_USERDATA`.
pub struct Platform {
    hwnd: usize,
    queue: Mutex<Option<Weak<Queue>>>,
    timers: Mutex<HashMap<usize, TimerSlot>>,
    next_timer: AtomicUsize,
}

impl Platform {
    /// Register the class once and create this thread's hidden window.
    pub fn create() -> Option<Arc<Self>> {
        static CLASS: OnceLock<bool> = OnceLock::new();
        let registered = *CLASS.get_or_init(|| {
            // Safety: the class name is static and the procedure has the system ABI.
            unsafe {
                let Ok(module) = GetModuleHandleW(None) else {
                    return false;
                };
                let class = WNDCLASSEXW {
                    cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    lpfnWndProc: Some(wndproc),
                    hInstance: module.into(),
                    lpszClassName: PCWSTR(CLASS_NAME.as_ptr()),
                    ..Default::default()
                };
                RegisterClassExW(&class) != 0
            }
        });
        if !registered {
            return None;
        }

        // Safety: the class is registered and no parent is wanted, which
        // `HWND_MESSAGE` spells as a message-only window.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(CLASS_NAME.as_ptr()),
                PCWSTR::null(),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )
            .ok()?
        };
        let platform = Arc::new(Self {
            hwnd: hwnd.0 as usize,
            queue: Mutex::new(None),
            timers: Mutex::new(HashMap::new()),
            next_timer: AtomicUsize::new(1),
        });
        // Safety: the pointer stays valid while the `Arc` is alive, and the
        // window is destroyed in `Drop` before the `Arc` goes away.
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Arc::as_ptr(&platform) as isize);
        }
        Some(platform)
    }

    pub fn set_queue(&self, queue: Weak<Queue>) {
        *self.queue.lock().unwrap() = Some(queue);
    }

    fn hwnd(&self) -> HWND {
        HWND(self.hwnd as *mut core::ffi::c_void)
    }

    /// Run the thread's loop until a quit is retrieved. `continue_on_quit`
    /// matches `DispatcherRunOptions::ContinueOnQuit`.
    pub fn run_loop(&self, continue_on_quit: bool) {
        loop {
            let mut message = MSG::default();
            // Safety: `message` is writable and no filter is wanted.
            let result = unsafe { GetMessageW(&mut message, None, 0, 0).0 };
            if result == -1 {
                return;
            }
            if result == 0 {
                if continue_on_quit {
                    continue;
                }
                return;
            }
            // Safety: `GetMessageW` filled the structure.
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    /// Register a timer slot and start Win32's timer on this thread.
    pub fn start_timer(&self, slot: TimerSlot, milliseconds: u32) -> Option<usize> {
        for _ in 0..8 {
            let id = self.next_timer.fetch_add(1, Ordering::Relaxed);
            if id == 0 {
                continue;
            }
            let mut timers = self.timers.lock().unwrap();
            if timers.contains_key(&id) {
                continue;
            }
            timers.insert(id, slot);
            drop(timers);
            // Safety: the window belongs to this thread and `id` is unique.
            let started = unsafe { SetTimer(Some(self.hwnd()), id, milliseconds, None) };
            if started != 0 {
                return Some(id);
            }
            self.timers.lock().unwrap().remove(&id);
            return None;
        }
        None
    }

    pub fn stop_timer(&self, id: usize) {
        self.timers.lock().unwrap().remove(&id);
        // Safety: the window is live on this thread.
        unsafe {
            let _ = KillTimer(Some(self.hwnd()), id);
        }
    }

    fn drain(&self) {
        let queue = self.queue.lock().unwrap().as_ref().and_then(Weak::upgrade);
        if let Some(queue) = queue {
            queue.run_pending();
        }
    }

    fn fire_timer(&self, id: usize) {
        let slot = self.timers.lock().unwrap().get(&id).cloned();
        if let Some(slot) = slot
            && !crate::timer::fire(&slot)
        {
            self.stop_timer(id);
        }
    }

    fn begin_quit(&self) {
        // Safety: the queue belongs to this thread, which is the one that must
        // receive the `WM_QUIT`. A cross-thread caller arrives through the
        // private message instead, so this runs on the owner either way.
        unsafe { PostQuitMessage(0) };
    }
}

impl Drop for Platform {
    fn drop(&mut self) {
        let hwnd = self.hwnd();
        for id in self
            .timers
            .lock()
            .unwrap()
            .keys()
            .copied()
            .collect::<Vec<_>>()
        {
            // Safety: the window is still alive; a wrong-thread call fails
            // harmlessly and the timer dies with the thread.
            unsafe {
                let _ = KillTimer(Some(hwnd), id);
            }
        }
        // Safety: destroying the window clears its user data with it.
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    }
}

impl Poster for Platform {
    fn post(&self) -> bool {
        // Safety: the window is alive for the queue that holds this `Arc`.
        unsafe { PostMessageW(Some(self.hwnd()), WM_DISPATCH, WPARAM(0), LPARAM(0)).is_ok() }
    }

    fn quit(&self) {
        // Safety: same lifetime as `post`.
        unsafe {
            let _ = PostMessageW(Some(self.hwnd()), WM_QUIT_LOCAL, WPARAM(0), LPARAM(0));
        }
    }
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Safety: the procedure is only installed on windows whose user data was
    // set to a live `Platform`, and the window is destroyed before that `Arc`.
    let platform = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Platform;
    if !platform.is_null() {
        let platform = unsafe { &*platform };
        match message {
            WM_DISPATCH => {
                platform.drain();
                return LRESULT(0);
            }
            WM_TIMER => {
                platform.fire_timer(wparam.0);
                return LRESULT(0);
            }
            WM_QUIT_LOCAL => {
                platform.begin_quit();
                return LRESULT(0);
            }
            _ => {}
        }
    }
    // Safety: the remaining messages are the default procedure's.
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}
