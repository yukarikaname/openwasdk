//! The presenter's tests: a real window, a real device, and the pixels it drew.

use super::*;
use opencomp_core::{Gradient, GradientShape, GradientStop, Rect, Shape};
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
        gradient: None,
        uv: [0.0, 0.0, 1.0, 1.0],
        size: [64.0, 64.0],
        radius: [8.0; 4],
        shape: None,
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
fn a_linear_gradient_runs_from_dark_to_light() {
    let hwnd = window();
    let Some(mut presenter) = Presenter::new(hwnd.0 as u64, 64, 64) else {
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        return;
    };
    let gradient = Gradient {
        id: 1,
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: [0.0, 0.0, 0.0, 1.0],
            },
            GradientStop {
                offset: 1.0,
                color: [1.0, 1.0, 1.0, 1.0],
            },
        ],
        shape: GradientShape::Linear {
            start: [0.0, 32.0],
            end: [64.0, 32.0],
        },
    };
    let draws = vec![Draw {
        corners: [[0.0, 0.0], [64.0, 0.0], [64.0, 64.0], [0.0, 64.0]],
        scissor: Rect::new(0.0, 0.0, 64.0, 64.0),
        // A green flat colour must not leak through: only the ramp should reach the pixels.
        color: [0.0, 1.0, 0.0, 1.0],
        image: None,
        gradient: Some(gradient),
        uv: [0.0, 0.0, 1.0, 1.0],
        size: [64.0, 64.0],
        radius: [0.0; 4],
        shape: None,
    }];
    assert!(presenter.draw(&draws));
    let dark = presenter.read_pixel(4, 32).expect("pixel");
    let light = presenter.read_pixel(60, 32).expect("pixel");
    assert!(
        dark[0] < 60 && dark[1] < 60 && dark[2] < 60,
        "start of the ramp was {dark:?}"
    );
    assert!(
        light[0] > 195 && light[1] > 195 && light[2] > 195,
        "end of the ramp was {light:?}"
    );
    unsafe {
        let _ = DestroyWindow(hwnd);
    }
}

#[test]
fn an_ellipse_paints_its_inside_and_leaves_the_corner() {
    let hwnd = window();
    let Some(mut presenter) = Presenter::new(hwnd.0 as u64, 64, 64) else {
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
        return;
    };
    // A red ellipse of radius 24 about the centre of the 64x64 surface.
    let draws = vec![Draw {
        corners: [[0.0, 0.0], [64.0, 0.0], [64.0, 64.0], [0.0, 64.0]],
        scissor: Rect::new(0.0, 0.0, 64.0, 64.0),
        color: [1.0, 0.0, 0.0, 1.0],
        image: None,
        gradient: None,
        uv: [0.0, 0.0, 1.0, 1.0],
        size: [64.0, 64.0],
        radius: [0.0; 4],
        shape: Some(Shape::Ellipse {
            center: [32.0, 32.0],
            radius: [24.0, 24.0],
        }),
    }];
    assert!(presenter.draw(&draws));
    let centre = presenter.read_pixel(32, 32).expect("pixel");
    assert!(
        centre[0] > 240 && centre[1] < 16 && centre[2] < 16,
        "{centre:?}"
    );
    // The corner is outside the ellipse and keeps the clear colour.
    let corner = presenter.read_pixel(1, 1).expect("pixel");
    assert!(
        corner[0] < 16 && corner[1] < 16 && corner[2] < 16,
        "{corner:?}"
    );
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
