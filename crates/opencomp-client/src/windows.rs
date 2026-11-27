//! The Windows half of the client: a window, a tree built through the ABI, and the bridge.
//!
//! Activation goes through `opencomp::DllGetActivationFactory`, the same entry point the DLL
//! exports, rather than through the Windows runtime. The factory for a class is then queried for
//! its statics interface, which is where `ContentIsland` and `DesktopChildSiteBridge` are
//! created; the generated bindings have no method wrappers for those statics, so the calls go
//! through the vtable.
//!
//! The tree is two sprites in a container whose size is relative to the island: a colour
//! rectangle with a rounded geometric clip, and a 64x64 checkerboard painted through a surface
//! brush. The bridge owns the window and starts its presenter thread on `Show`.

use std::ffi::c_void;

use opencomp::bindings::Microsoft::UI::Composition::{
    CompositionBrush, CompositionClip, CompositionGeometry, CompositionRoundedRectangleGeometry,
    ICompositionSurface, ICompositor, ICompositor5, ICompositor6, IContainerVisual, ISpriteVisual,
    IVisual, IVisual2, Visual,
};
use opencomp::bindings::Microsoft::UI::Content::{
    ContentIsland, DesktopChildSiteBridge, IContentIsland, IContentIslandStatics,
    IDesktopChildSiteBridgeStatics, IDesktopSiteBridge,
};
use opencomp::bindings::Microsoft::UI::WindowId;
use opencomp::bindings::Windows::Foundation::IClosable;
use opencomp::bindings::Windows::Graphics::RectInt32;
use opencomp::bindings::Windows::UI::Color;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::Sleep;
use windows::Win32::UI::WindowsAndMessaging::{
    CW_USEDEFAULT, CreateWindowExW, DestroyWindow, DispatchMessageW, MSG, PM_REMOVE, PeekMessageW,
    SW_SHOW, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WS_OVERLAPPEDWINDOW,
};
use windows::core::w;
use windows_core::{HSTRING, IUnknown, Interface};
use windows_numerics::{Vector2, Vector3};

/// The runtime classes this DLL registers; the names are matched in `opencomp::lib`.
const COMPOSITOR: &str = "Microsoft.UI.Composition.Compositor";
const CONTENT_ISLAND: &str = "Microsoft.UI.Content.ContentIsland";
const SITE_BRIDGE: &str = "Microsoft.UI.Content.DesktopChildSiteBridge";

const WIDTH: i32 = 320;
const HEIGHT: i32 = 240;
const FRAME_MS: u32 = 16;
const DEFAULT_FRAMES: u32 = 120;

const E_FAIL: i32 = 0x8000_4005u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;

pub fn run() -> i32 {
    let hwnd = window();
    let compositor = compositor();
    let island = island(&compositor);
    let bridge = bridge(&compositor, hwnd);

    let site: IDesktopSiteBridge = step("QueryInterface(IDesktopSiteBridge)", bridge.cast());
    let rect = RectInt32 {
        x: 0,
        y: 0,
        width: WIDTH,
        height: HEIGHT,
    };
    step("MoveAndResize", site.MoveAndResize(rect));
    step("Connect", site.Connect(&island));
    // Before `Connect` the rectangle is only the bridge's; the second call copies it into the
    // island, which is what `ActualSize` and the snapshot read.
    step("MoveAndResize(connected)", site.MoveAndResize(rect));
    step("Show", site.Show());

    // The first frame costs a device, a swapchain and a D2D target; wait for it rather
    // than for a fixed number of iterations.
    let frames = frames();
    pump_until_presented(frames);

    step("Hide", site.Hide());
    let closable: IClosable = step("QueryInterface(IClosable)", bridge.cast());
    step("Close", closable.Close());
    if let Err(error) = unsafe { DestroyWindow(hwnd) } {
        fail("DestroyWindow", error.code().0);
    }

    let state: IContentIsland = step("QueryInterface(IContentIsland)", island.cast());
    let size = step("ActualSize", state.ActualSize());
    let presented = opencomp::frames_presented();
    if presented == 0 {
        eprintln!(
            "opencomp-client: no frame reached the window; the device or the swapchain failed"
        );
        std::process::exit(1);
    }
    println!(
        "presented {frames} frames through DesktopChildSiteBridge on a {}x{} island",
        size.x, size.y
    );
    0
}

/// A 320x240 window of the built-in STATIC class, shown.
fn window() -> HWND {
    trace("CreateWindowExW");
    let hwnd = match unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("opencomp client"),
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
    } {
        Ok(hwnd) => hwnd,
        Err(error) => fail("CreateWindowExW", error.code().0),
    };
    let _ = unsafe { ShowWindow(hwnd, SW_SHOW) };
    hwnd
}

/// The compositor, activated through the export every class in this DLL answers.
fn compositor() -> ICompositor {
    let factory = factory(COMPOSITOR);
    let activation: windows_core::imp::IGenericFactory =
        step("QueryInterface(IActivationFactory)", factory.cast());
    step(
        "ActivateInstance(Compositor)",
        activation.ActivateInstance(),
    )
}

/// The island around a freshly built tree.
fn island(compositor: &ICompositor) -> ContentIsland {
    let root = tree(compositor);
    let factory = factory(CONTENT_ISLAND);
    let statics: IContentIslandStatics =
        step("QueryInterface(IContentIslandStatics)", factory.cast());

    // `IContentIslandStatics` has no generated method wrapper; the vtable slot takes a borrowed
    // `Visual`, so the root stays owned here.
    let mut raw: *mut c_void = std::ptr::null_mut();
    let result = unsafe {
        (Interface::vtable(&statics).Create)(
            Interface::as_raw(&statics),
            Interface::as_raw(&root),
            &mut raw,
        )
    };
    check("ContentIsland::Create", result);
    owned("ContentIsland::Create", raw)
}

/// The bridge for `hwnd`; `run` connects the island and sets the rectangle.
fn bridge(compositor: &ICompositor, hwnd: HWND) -> DesktopChildSiteBridge {
    let factory = factory(SITE_BRIDGE);
    let statics: IDesktopChildSiteBridgeStatics = step(
        "QueryInterface(IDesktopChildSiteBridgeStatics)",
        factory.cast(),
    );

    let mut raw: *mut c_void = std::ptr::null_mut();
    let result = unsafe {
        (Interface::vtable(&statics).Create)(
            Interface::as_raw(&statics),
            Interface::as_raw(compositor),
            WindowId {
                value: hwnd.0 as usize as u64,
            },
            &mut raw,
        )
    };
    check("DesktopChildSiteBridge::Create", result);
    owned("DesktopChildSiteBridge::Create", raw)
}

/// Build the tree and return its root.
fn tree(compositor: &ICompositor) -> Visual {
    let root = step("CreateContainerVisual", compositor.CreateContainerVisual());
    let relative: IVisual2 = step("QueryInterface(IVisual2)", root.cast());
    step(
        "SetRelativeSizeAdjustment",
        relative.SetRelativeSizeAdjustment(Vector2 { x: 1.0, y: 1.0 }),
    );
    let children: IContainerVisual = step("QueryInterface(IContainerVisual)", root.cast());
    let collection = step("Children", children.Children());

    let colour = step(
        "CreateColorBrushWithColor",
        compositor.CreateColorBrushWithColor(Color {
            a: 255,
            r: 64,
            g: 160,
            b: 96,
        }),
    );
    let colour: CompositionBrush = step("QueryInterface(CompositionBrush)", colour.cast());
    let sprite = step("CreateSpriteVisual", compositor.CreateSpriteVisual());
    let visual: IVisual = step("QueryInterface(IVisual)", sprite.cast());
    step(
        "SetOffset(colour)",
        visual.SetOffset(Vector3 {
            x: 16.0,
            y: 16.0,
            z: 0.0,
        }),
    );
    step(
        "SetSize(colour)",
        visual.SetSize(Vector2 { x: 176.0, y: 176.0 }),
    );
    let clip = rounded_clip(compositor, 176.0, 24.0);
    step("SetClip", visual.SetClip(&clip));
    let sprite_visual: ISpriteVisual = step("QueryInterface(ISpriteVisual)", sprite.cast());
    step("SetBrush(colour)", sprite_visual.SetBrush(&colour));
    step("InsertAtTop(colour)", collection.InsertAtTop(&sprite));

    let drawing = step("surface::create", opencomp::surface::create(64, 64));
    if !opencomp::surface::surface_update(&drawing, &checkerboard()) {
        fail("surface::surface_update", E_FAIL);
    }
    let surface: ICompositionSurface = step("QueryInterface(ICompositionSurface)", drawing.cast());
    let brush = step(
        "CreateSurfaceBrushWithSurface",
        compositor.CreateSurfaceBrushWithSurface(&surface),
    );
    let brush: CompositionBrush = step("QueryInterface(CompositionBrush)", brush.cast());
    let image = step("CreateSpriteVisual", compositor.CreateSpriteVisual());
    let visual: IVisual = step("QueryInterface(IVisual)", image.cast());
    step(
        "SetOffset(image)",
        visual.SetOffset(Vector3 {
            x: 224.0,
            y: 152.0,
            z: 0.0,
        }),
    );
    step(
        "SetSize(image)",
        visual.SetSize(Vector2 { x: 64.0, y: 64.0 }),
    );
    let sprite_visual: ISpriteVisual = step("QueryInterface(ISpriteVisual)", image.cast());
    step("SetBrush(image)", sprite_visual.SetBrush(&brush));
    step("InsertAtTop(image)", collection.InsertAtTop(&image));

    step("QueryInterface(Visual)", root.cast())
}

/// A geometric clip rounded by `radius` over a square of `size`.
fn rounded_clip(compositor: &ICompositor, size: f32, radius: f32) -> CompositionClip {
    let five: ICompositor5 = step("QueryInterface(ICompositor5)", compositor.cast());
    let geometry: CompositionRoundedRectangleGeometry = step(
        "CreateRoundedRectangleGeometry",
        five.CreateRoundedRectangleGeometry(),
    );
    step(
        "SetOffset(geometry)",
        geometry.SetOffset(Vector2 { x: 0.0, y: 0.0 }),
    );
    step(
        "SetSize(geometry)",
        geometry.SetSize(Vector2 { x: size, y: size }),
    );
    step(
        "SetCornerRadius",
        geometry.SetCornerRadius(Vector2 {
            x: radius,
            y: radius,
        }),
    );

    let geometry: CompositionGeometry =
        step("QueryInterface(CompositionGeometry)", geometry.cast());
    let six: ICompositor6 = step("QueryInterface(ICompositor6)", compositor.cast());
    let clip = step(
        "CreateGeometricClipWithGeometry",
        six.CreateGeometricClipWithGeometry(&geometry),
    );
    step("QueryInterface(CompositionClip)", clip.cast())
}

/// An 8 pixel checkerboard, sRGB RGBA8 and row major, as `surface_update` expects.
fn checkerboard() -> Vec<u8> {
    const SIDE: usize = 64;
    const CELL: usize = 8;
    const LIGHT: [u8; 4] = [232, 232, 232, 255];
    const DARK: [u8; 4] = [40, 40, 40, 255];

    let mut pixels = vec![0u8; SIDE * SIDE * 4];
    for y in 0..SIDE {
        for x in 0..SIDE {
            let colour = if (x / CELL + y / CELL).is_multiple_of(2) {
                LIGHT
            } else {
                DARK
            };
            let at = (y * SIDE + x) * 4;
            pixels[at..at + 4].copy_from_slice(&colour);
        }
    }
    pixels
}

/// Drain the window's messages once per iteration, then let the presenter draw.
fn pump_until_presented(frames: u32) {
    let mut message = MSG::default();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut ticks = 0;
    while ticks < frames || opencomp::frames_presented() == 0 {
        unsafe {
            while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&message);
                let _ = DispatchMessageW(&message);
            }
            Sleep(FRAME_MS);
        }
        ticks += 1;
        // The first frame needs a device and a target; after two seconds the graphics
        // stack is not going to answer and the caller reports it.
        if std::time::Instant::now() > deadline {
            break;
        }
    }
}

/// Ask the DLL for the activation factory of `class`.
fn factory(class: &str) -> IUnknown {
    let name = HSTRING::from(class);
    let mut raw: *mut c_void = std::ptr::null_mut();
    // The parameter is the HSTRING handle itself, which is the pointer inside the HSTRING.
    let result = unsafe { opencomp::DllGetActivationFactory(handle(&name), &mut raw) };
    if result < 0 {
        fail(&format!("DllGetActivationFactory({class})"), result);
    }
    owned(class, raw)
}

fn frames() -> u32 {
    std::env::var("OPENCOMP_FRAMES")
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(DEFAULT_FRAMES)
}

fn handle(name: &HSTRING) -> *mut c_void {
    unsafe { std::mem::transmute_copy(name) }
}

/// Print every step when `OPENCOMP_TRACE` is set.
///
/// stdout is block-buffered when it is a pipe, so a crash loses every `println!`
/// that came before it; stderr is not, and a trace is what turns "page fault" into
/// "the page fault happened at this call".
fn trace(name: &str) {
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!("opencomp-client: {name}");
    }
}

fn step<T>(name: &str, result: windows_core::Result<T>) -> T {
    trace(name);
    match result {
        Ok(value) => value,
        Err(error) => fail(name, error.code().0),
    }
}

fn check(name: &str, result: windows_core::HRESULT) {
    trace(name);
    if result.0 < 0 {
        fail(name, result.0);
    }
}

fn owned<T: Interface>(name: &str, raw: *mut c_void) -> T {
    trace(name);
    if raw.is_null() {
        fail(name, E_POINTER);
    }
    unsafe { T::from_raw(raw) }
}

/// Report the step that returned a failing HRESULT and exit 1.
fn fail(name: &str, code: i32) -> ! {
    eprintln!("opencomp-client: {name} failed: 0x{:08x}", code as u32);
    std::process::exit(1);
}
