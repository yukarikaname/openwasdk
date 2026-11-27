//! Gate A: a window, a tree, an island, a bridge, and a frame.
//!
//! The client binary runs the same path, but a panic there is a page fault with no
//! symbol. As a test it is a Rust panic with a backtrace, and it runs both on Windows
//! and under Wine in CI, which is where the graphics stack differs.

#![cfg(windows)]

use std::ffi::c_void;
use std::time::Duration;

use opencomp::bindings::Microsoft::UI::Composition::{
    ICompositor, IContainerVisual, IVisual, IVisual2,
};
use opencomp::bindings::Microsoft::UI::Content::{
    IContentIslandStatics, IDesktopChildSiteBridgeStatics, IDesktopSiteBridge,
};
use opencomp::bindings::Microsoft::UI::WindowId;
use opencomp::bindings::Windows::Graphics::RectInt32;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    CW_USEDEFAULT, CreateWindowExW, DestroyWindow, DispatchMessageW, MSG, PM_REMOVE, PeekMessageW,
    TranslateMessage, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
};
use windows::core::w;
use windows_core::{HSTRING, IUnknown, Interface};
use windows_numerics::Vector2;

const WIDTH: i32 = 160;
const HEIGHT: i32 = 120;

fn handle(name: &HSTRING) -> *mut c_void {
    unsafe { std::mem::transmute_copy(name) }
}

/// The activation factory for one of our classes.
fn factory(class: &str) -> IUnknown {
    let name = HSTRING::from(class);
    let mut raw: *mut c_void = std::ptr::null_mut();
    // Safety: the parameter is the HSTRING handle itself and `raw` is writable.
    let result = unsafe { opencomp::DllGetActivationFactory(handle(&name), &mut raw) };
    assert!(
        result >= 0,
        "DllGetActivationFactory({class}) = {result:#x}"
    );
    assert!(!raw.is_null(), "{class} produced no factory");
    unsafe { IUnknown::from_raw(raw) }
}

fn compositor() -> ICompositor {
    let activation: windows_core::imp::IGenericFactory =
        factory("Microsoft.UI.Composition.Compositor")
            .cast()
            .expect("IActivationFactory");
    activation.ActivateInstance().expect("compositor")
}

/// A hidden window of the built-in STATIC class.
fn window() -> HWND {
    // Safety: the STATIC class needs no registration.
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("opencomp gate a"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            WIDTH,
            HEIGHT,
            None,
            None,
            None,
            None,
        )
        .expect("window")
    }
}

#[test]
fn a_window_presents_a_frame() {
    let hwnd = window();
    let compositor = compositor();

    // A root that fills the island, with one red sprite in it.
    let root = compositor.CreateContainerVisual().expect("container");
    let visual: IVisual2 = root.cast().expect("IVisual2");
    visual
        .SetRelativeSizeAdjustment(Vector2 { x: 1.0, y: 1.0 })
        .expect("relative size");
    let sprite = compositor.CreateSpriteVisual().expect("sprite");
    let sprite_visual: IVisual = sprite.cast().expect("IVisual");
    sprite_visual
        .SetSize(Vector2 {
            x: WIDTH as f32,
            y: HEIGHT as f32,
        })
        .expect("size");
    let brush = compositor
        .CreateColorBrushWithColor(opencomp::bindings::Windows::UI::Color {
            a: 255,
            r: 220,
            g: 60,
            b: 60,
        })
        .expect("brush");
    let sprite_surface: opencomp::bindings::Microsoft::UI::Composition::ISpriteVisual =
        sprite.cast().expect("ISpriteVisual");
    let brush: opencomp::bindings::Microsoft::UI::Composition::CompositionBrush =
        brush.cast().expect("CompositionBrush");
    sprite_surface.SetBrush(&brush).expect("set brush");
    let children: IContainerVisual = root.cast().expect("IContainerVisual");
    children
        .Children()
        .expect("children")
        .InsertAtTop(&sprite)
        .expect("insert");

    let root_visual: opencomp::bindings::Microsoft::UI::Composition::Visual =
        root.cast().expect("Visual");

    let island_statics: IContentIslandStatics = factory("Microsoft.UI.Content.ContentIsland")
        .cast()
        .expect("statics");
    let mut island_raw: *mut c_void = std::ptr::null_mut();
    // Safety: the out pointer is ours and the statics object is alive.
    let island = unsafe {
        (Interface::vtable(&island_statics).Create)(
            Interface::as_raw(&island_statics),
            Interface::as_raw(&root_visual),
            &mut island_raw,
        )
    };
    assert!(island.0 >= 0, "ContentIsland::Create = {:#x}", island.0);
    assert!(!island_raw.is_null(), "no island");

    let bridge_statics: IDesktopChildSiteBridgeStatics =
        factory("Microsoft.UI.Content.DesktopChildSiteBridge")
            .cast()
            .expect("statics");
    let mut raw: *mut c_void = std::ptr::null_mut();
    // Safety: the window outlives the bridge, which is dropped before it.
    let result = unsafe {
        (Interface::vtable(&bridge_statics).Create)(
            Interface::as_raw(&bridge_statics),
            Interface::as_raw(&compositor),
            WindowId {
                value: hwnd.0 as usize as u64,
            },
            &mut raw,
        )
    };
    assert!(
        result.0 >= 0,
        "DesktopChildSiteBridge::Create = {:#x}",
        result.0
    );
    let bridge: IDesktopSiteBridge = unsafe {
        opencomp::bindings::Microsoft::UI::Content::DesktopChildSiteBridge::from_raw(raw)
    }
    .cast()
    .expect("IDesktopSiteBridge");

    let rect = RectInt32 {
        x: 0,
        y: 0,
        width: WIDTH,
        height: HEIGHT,
    };
    bridge.MoveAndResize(rect).expect("resize");
    bridge
        .Connect(&unsafe {
            opencomp::bindings::Microsoft::UI::Content::ContentIsland::from_raw(island_raw)
        })
        .expect("connect");
    bridge.MoveAndResize(rect).expect("resize after connect");
    bridge.Show().expect("show");

    let mut message = MSG::default();
    for _ in 0..10 {
        // Safety: the message lives on this stack frame.
        if unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        std::thread::sleep(Duration::from_millis(16));
    }

    bridge.Hide().expect("hide");
    let presented = opencomp::frames_presented();
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
    assert!(presented > 0, "no frame reached the window");
}
