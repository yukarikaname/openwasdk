//! The presenter's tests: a real window, a real device, and the pixels it drew.

use super::*;
use opencomp_core::Rect;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, WS_OVERLAPPEDWINDOW,
};
use windows::core::w;

/// A window that is created and never shown: the compositor only needs the handle,
/// and Xvfb in CI has no one to look at it.
fn window() -> HWND {
    // Safety: the built-in STATIC class needs no registration.
    unsafe {
        CreateWindowExW(
            Default::default(),
            w!("STATIC"),
            w!("opencomp test"),
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            64,
            64,
            None,
            None,
            None,
            None,
        )
        .expect("window")
    }
}

fn draws() -> Vec<Draw> {
    vec![Draw {
        corners: [[0.0, 0.0], [64.0, 0.0], [64.0, 64.0], [0.0, 64.0]],
        scissor: Rect::new(0.0, 0.0, 64.0, 64.0),
        color: [1.0, 0.0, 0.0, 1.0],
        image: None,
        uv: [0.0, 0.0, 1.0, 1.0],
        size: [64.0, 64.0],
        radius: [8.0; 4],
    }]
}

#[test]
fn a_window_presents_frames() {
    let hwnd = window();
    let Some(mut presenter) = Presenter::new(hwnd.0 as u64, 64, 64) else {
        // No device under this Wine prefix; nothing to say.
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        return;
    };
    assert!(presenter.present(&draws()));
    assert!(presenter.present(&draws()));
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
}

#[test]
fn a_drawn_frame_has_the_colours_it_was_given() {
    let hwnd = window();
    let Some(mut presenter) = Presenter::new(hwnd.0 as u64, 64, 64) else {
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        return;
    };
    // The list is one red rounded rectangle with an 8 unit radius over 64 units.
    assert!(presenter.draw(&draws()));
    let centre = presenter.read_pixel(32, 32).expect("pixel");
    assert!(
        centre[0] > 240 && centre[1] < 16 && centre[2] < 16,
        "{centre:?}"
    );
    // A radius of 8 removes the corner, which keeps the clear colour.
    let corner = presenter.read_pixel(1, 1).expect("pixel");
    assert!(
        corner[0] < 16 && corner[1] < 16 && corner[2] < 16,
        "{corner:?}"
    );
    assert!(presenter.present_frame());
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
}

#[test]
fn a_resize_keeps_presenting() {
    let hwnd = window();
    let Some(mut presenter) = Presenter::new(hwnd.0 as u64, 64, 64) else {
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        return;
    };
    assert!(presenter.resize(32, 48));
    assert_eq!(presenter.size(), (32, 48));
    assert!(presenter.present(&draws()));
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
}
