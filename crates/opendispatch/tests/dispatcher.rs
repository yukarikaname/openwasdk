//! What needs a real message loop: the private window message, the Win32 timer
//! and the shutdown quit. These run on Windows in CI; the queue's ordering and
//! thread comparison are tested on every host in `src/queue.rs`.

#![cfg(windows)]

use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use opendispatch::bindings::Microsoft::UI::Dispatching::{
    DispatcherQueue, DispatcherQueueController, DispatcherQueueHandler, DispatcherQueuePriority,
    DispatcherQueueTimer, IDispatcherQueue2, IDispatcherQueue3, IDispatcherQueueControllerStatics,
    IDispatcherQueueStatics,
};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage, WM_QUIT,
};
use windows_core::{HSTRING, IUnknown, Interface};
use windows_time::TimeSpan;

fn handle(name: &HSTRING) -> *mut c_void {
    unsafe { std::mem::transmute_copy(name) }
}

fn factory(class: &str) -> IUnknown {
    let name = HSTRING::from(class);
    let mut raw: *mut c_void = std::ptr::null_mut();
    // Safety: the name is live across the call and `raw` is writable.
    let hr = unsafe { opendispatch::DllGetActivationFactory(handle(&name), &mut raw) };
    assert_eq!(hr, 0, "DllGetActivationFactory({class})");
    assert!(!raw.is_null());
    unsafe { IUnknown::from_raw(raw) }
}

fn create_on_current(statics: &IDispatcherQueueControllerStatics) -> DispatcherQueueController {
    let mut raw: *mut c_void = std::ptr::null_mut();
    // Safety: the statics object is live and `raw` is writable.
    let hr = unsafe {
        (Interface::vtable(statics).CreateOnCurrentThread)(Interface::as_raw(statics), &mut raw)
    };
    assert!(hr.is_ok());
    assert!(!raw.is_null());
    unsafe { DispatcherQueueController::from_raw(raw) }
}

fn create_on_dedicated(statics: &IDispatcherQueueControllerStatics) -> DispatcherQueueController {
    let mut raw: *mut c_void = std::ptr::null_mut();
    // Safety: the statics object is live and `raw` is writable.
    let hr = unsafe {
        (Interface::vtable(statics).CreateOnDedicatedThread)(Interface::as_raw(statics), &mut raw)
    };
    assert!(hr.is_ok());
    assert!(!raw.is_null());
    unsafe { DispatcherQueueController::from_raw(raw) }
}

fn queue_for_current_thread(statics: &IDispatcherQueueStatics) -> DispatcherQueue {
    let mut raw: *mut c_void = std::ptr::null_mut();
    // Safety: the statics object is live and `raw` is writable.
    let hr = unsafe {
        (Interface::vtable(statics).GetForCurrentThread)(Interface::as_raw(statics), &mut raw)
    };
    assert!(hr.is_ok());
    assert!(!raw.is_null(), "the controller installed a queue");
    unsafe { DispatcherQueue::from_raw(raw) }
}

fn controller() -> DispatcherQueueController {
    let statics: IDispatcherQueueControllerStatics =
        factory("Microsoft.UI.Dispatching.DispatcherQueueController")
            .cast()
            .expect("controller statics");
    create_on_current(&statics)
}

/// Run the thread's message loop until `done` or the deadline. A `WM_QUIT`
/// ends the loop the way it ends a caller's loop.
fn pump_until(done: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !done() && Instant::now() < deadline {
        let mut message = MSG::default();
        // Safety: `message` is writable and no filter is wanted.
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() } {
            if message.message == WM_QUIT {
                return done();
            }
            // Safety: `PeekMessageW` filled the structure.
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    done()
}

#[test]
fn a_queue_posts_work_back_to_its_own_thread() {
    let controller = controller();
    let queue = controller.DispatcherQueue().expect("queue");
    let queue2: IDispatcherQueue2 = queue.cast().expect("IDispatcherQueue2");
    assert!(queue2.HasThreadAccess().unwrap());

    let log = Arc::new(std::sync::Mutex::new(Vec::new()));
    let creator = unsafe { GetCurrentThreadId() };
    let seen = Arc::new(AtomicU32::new(0));

    for (priority, name) in [
        (DispatcherQueuePriority::Normal, "normal"),
        (DispatcherQueuePriority::Low, "low"),
        (DispatcherQueuePriority::High, "high"),
    ] {
        let log = log.clone();
        let seen = seen.clone();
        let handler = DispatcherQueueHandler::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
            let current = unsafe { GetCurrentThreadId() };
            log.lock().unwrap().push((name, current == creator));
        });
        assert!(queue.TryEnqueueWithPriority(priority, &handler).unwrap());
    }

    assert!(pump_until(|| seen.load(Ordering::SeqCst) == 3));
    let log = log.lock().unwrap().clone();
    let names: Vec<&str> = log.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, ["high", "normal", "low"]);
    assert!(log.iter().all(|(_, same_thread)| *same_thread));

    // The statics class finds the queue the controller installed.
    let statics: IDispatcherQueueStatics = factory("Microsoft.UI.Dispatching.DispatcherQueue")
        .cast()
        .unwrap();
    let current = queue_for_current_thread(&statics);
    assert!(
        current
            .cast::<IDispatcherQueue2>()
            .unwrap()
            .HasThreadAccess()
            .unwrap()
    );
}

#[test]
fn a_timer_ticks_on_the_message_loop() {
    let controller = controller();
    let queue = controller.DispatcherQueue().expect("queue");
    let timer: DispatcherQueueTimer = queue.CreateTimer().expect("timer");
    timer
        .SetInterval(TimeSpan { duration: 10_000 })
        .expect("interval");
    timer.SetIsRepeating(false).expect("one shot");

    let (sender, receiver) = mpsc::channel();
    let _revoker = timer
        .Tick(move |_, _| {
            let _ = sender.send(());
        })
        .expect("tick handler");
    timer.Start().expect("start");
    assert!(timer.IsRunning().unwrap());

    assert!(pump_until(|| receiver.try_recv().is_ok()));
    assert!(!timer.IsRunning().unwrap(), "a one-shot timer stops itself");
}

#[test]
fn shutdown_raises_the_event_and_refuses_more_work() {
    let controller = controller();
    let queue = controller.DispatcherQueue().expect("queue");
    let called = Arc::new(AtomicBool::new(false));
    let flag = called.clone();
    let _revoker = queue
        .ShutdownStarting(move |_, _| flag.store(true, Ordering::SeqCst))
        .expect("shutdown handler");

    let action = controller.ShutdownQueueAsync().expect("shutdown action");
    drop(action);
    assert!(pump_until(|| called.load(Ordering::SeqCst)));

    let handler = DispatcherQueueHandler::new(|| {});
    assert!(!queue.TryEnqueue(&handler).unwrap(), "queue is closed");
}

#[test]
fn a_dedicated_queue_runs_off_the_creating_thread() {
    let statics: IDispatcherQueueControllerStatics =
        factory("Microsoft.UI.Dispatching.DispatcherQueueController")
            .cast()
            .unwrap();
    let controller = create_on_dedicated(&statics);
    let queue = controller.DispatcherQueue().expect("queue");
    let (sender, receiver) = mpsc::channel();
    let creator = unsafe { GetCurrentThreadId() };

    let handler = DispatcherQueueHandler::new(move || {
        let current = unsafe { GetCurrentThreadId() };
        let _ = sender.send(current);
    });
    queue.TryEnqueue(&handler).expect("enqueue");

    let worker = receiver.recv_timeout(Duration::from_secs(3)).expect("ran");
    assert_ne!(worker, creator, "the task ran on the dedicated thread");

    let controller2: opendispatch::bindings::Microsoft::UI::Dispatching::IDispatcherQueueController2 =
        controller.cast().unwrap();
    controller2.ShutdownQueue().expect("shutdown");
}

#[test]
fn an_unrelated_class_is_refused_by_the_export() {
    let name = HSTRING::from("Microsoft.UI.Xaml.Application");
    let mut raw: *mut c_void = std::ptr::null_mut();
    let hr = unsafe { opendispatch::DllGetActivationFactory(handle(&name), &mut raw) };
    assert_eq!(hr, 0x8004_0111u32 as i32);
    assert!(raw.is_null());
}

/// The queue's `IDispatcherQueue3` loop is the only one that owns the thread; a
/// test that used it would never return, so its two non-looping members are
/// called instead.
#[test]
fn the_queue3_surface_is_reachable() {
    let controller = controller();
    let queue: DispatcherQueue = controller.DispatcherQueue().unwrap();
    let queue3: IDispatcherQueue3 = queue.cast().unwrap();
    queue3.EnsureSystemDispatcherQueue().unwrap();
}
