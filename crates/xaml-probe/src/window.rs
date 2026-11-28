//! Gate B's second question: does the engine's own window reach the screen?
//!
//! `start.rs` proves the engine runs and can measure; this asks whether a `Window` it
//! creates -- with its own hosting, its own HWND, its own content path -- ends up
//! presenting through the compositor we serve. The frame counter is the answer, and it
//! comes from the `opencomp.dll` the engine loaded rather than from a linked copy.

use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage,
};

use crate::start::{
    GetActivationFactory, IPanelChildren, activation_factory, cast, check, fail, mark,
};
use crate::windows::IActivationFactory;
use crate::xaml_bindings::Microsoft::UI::Xaml::Media::ISolidColorBrush;
use crate::xaml_bindings::Microsoft::UI::Xaml::{IWindow, UIElement};
use crate::xaml_bindings::Windows::UI::Color;

/// Show a window with a painted background and pump it.
///
/// This is Gate B's end state: the engine creates its own window and hosts content, and
/// the question is whether anything of ours presents into it. The frame counter comes
/// from the `opencomp.dll` the engine actually loaded, not from a linked copy.
pub(crate) fn show(get: GetActivationFactory) {
    mark("window");
    let factory = activation_factory(get, "Microsoft.UI.Xaml.Window");
    let activation: IActivationFactory = cast(&factory, "IActivationFactory");
    let window: IWindow = cast(
        &activation
            .activate()
            .unwrap_or_else(|error| fail("Window.ActivateInstance", error.code().0)),
        "QueryInterface(IWindow)",
    );

    if std::env::var_os("OPENCOMP_XAML_PROBE_EMPTY").is_some() {
        // A window with no content isolates the engine's own hosting path from the first
        // `UIElement` a client builds.
        mark("window activate");
        window
            .Activate()
            .unwrap_or_else(|error| fail("Window.Activate", error.code().0));
        pump();
        return;
    }

    mark("window content");
    let grid_factory = activation_factory(get, "Microsoft.UI.Xaml.Controls.Grid");
    let grid_activation: IActivationFactory = cast(&grid_factory, "IActivationFactory");
    let grid = grid_activation
        .activate()
        .unwrap_or_else(|error| fail("Grid.ActivateInstance", error.code().0));
    let panel: IPanelChildren = cast(&grid, "QueryInterface(IPanel)");
    let brush_factory = activation_factory(get, "Microsoft.UI.Xaml.Media.SolidColorBrush");
    let brush_activation: IActivationFactory = cast(&brush_factory, "IActivationFactory");
    let brush = brush_activation
        .activate()
        .unwrap_or_else(|error| fail("SolidColorBrush.ActivateInstance", error.code().0));
    let color: ISolidColorBrush = cast(&brush, "QueryInterface(ISolidColorBrush)");
    color
        .SetColor(Color {
            a: 255,
            r: 220,
            g: 60,
            b: 60,
        })
        .unwrap_or_else(|error| fail("SolidColorBrush.Color", error.code().0));
    check("IPanel.Background", panel.set_background(&brush));

    let element: UIElement = cast(&grid, "QueryInterface(UIElement)");

    // `WindowChrome::OnContentChanged` expects the window to be inside a XamlIslandRoot
    // already, and the island is created when the window is activated. Windows has one by
    // the time content is set; whether that holds here is what the flag measures.
    let activate_first = std::env::var_os("OPENCOMP_XAML_PROBE_ACTIVATE_FIRST").is_some();
    if activate_first {
        mark("window activate");
        window
            .Activate()
            .unwrap_or_else(|error| fail("Window.Activate", error.code().0));
    }

    window
        .SetContent(&element)
        .unwrap_or_else(|error| fail("Window.Content", error.code().0));

    if !activate_first {
        mark("window activate");
        window
            .Activate()
            .unwrap_or_else(|error| fail("Window.Activate", error.code().0));
    }

    match window.Bounds() {
        Ok(bounds) => eprintln!(
            "xaml-probe: window bounds {}x{} at {},{}",
            bounds.width, bounds.height, bounds.x, bounds.y
        ),
        Err(error) => eprintln!("xaml-probe: Window.Bounds hr=0x{:08x}", error.code().0),
    }

    pump();
}

/// Pump the message queue for the window's lifetime, then report the frame count.
fn pump() {
    mark("window pump");
    let mut message = MSG::default();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        // Safety: `message` lives on this stack frame.
        while unsafe { PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0 {
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    eprintln!(
        "xaml-probe: frames presented by opencomp.dll: {}",
        frames_presented()
    );
}

/// The frame counter of the compositor the engine loaded, if the file is beside it.
fn frames_presented() -> u64 {
    let Some(path) = std::env::var_os("XAML_DLL") else {
        return 0;
    };
    let path = std::path::PathBuf::from(path).with_file_name("opencomp.dll");
    let Some(module) = crate::windows::load(&path) else {
        return 0;
    };
    let Some(entry) = crate::windows::export(module, "opencomp_frames_presented") else {
        return 0;
    };
    let frames: unsafe extern "system" fn() -> u64 = unsafe { std::mem::transmute(entry) };
    unsafe { frames() }
}
