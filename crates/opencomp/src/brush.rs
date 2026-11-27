//! `CompositionColorBrush`, the only brush that paints anything yet.
//!
//! The colour is copied into the visual when the brush is attached, so a `SetColor` after
//! `SetBrush` does not show up. That is the one visible shortcut here; real composition keeps a
//! live link.

use std::sync::Mutex;

use crate::bindings::Microsoft::UI::Composition::{
    CompositionBrush, CompositionColorBrush, ICompositionBrush, ICompositionBrush_Impl,
    ICompositionColorBrush, ICompositionColorBrush_Impl, ICompositionObject, ICompositionObject2,
    ICompositionObject4, ICompositionObject5,
};
use crate::bindings::Windows::UI::Color;
use crate::object::composition_object;
use windows_core::Result;

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionBrush,
    ICompositionColorBrush
)]
pub struct ColorBrush {
    color: Mutex<[f32; 4]>,
}

composition_object!(ColorBrush_Impl);

impl ICompositionBrush_Impl for ColorBrush_Impl {}

impl ICompositionColorBrush_Impl for ColorBrush_Impl {
    fn Color(&self) -> Result<Color> {
        Ok(from_rgba(*self.color.lock().unwrap()))
    }

    fn SetColor(&self, value: &Color) -> Result<()> {
        *self.color.lock().unwrap() = to_rgba(*value);
        Ok(())
    }
}

pub fn create() -> Result<CompositionColorBrush> {
    create_with(Color {
        a: 0,
        r: 0,
        g: 0,
        b: 0,
    })
}

pub fn create_with(color: Color) -> Result<CompositionColorBrush> {
    let brush: ICompositionColorBrush = ColorBrush {
        color: Mutex::new(to_rgba(color)),
    }
    .into();
    windows_core::Interface::cast(&brush)
}

/// A brush that paints nothing, for a sprite that has none.
pub fn transparent() -> Result<CompositionBrush> {
    let brush: ICompositionColorBrush = ColorBrush {
        color: Mutex::new([0.0; 4]),
    }
    .into();
    windows_core::Interface::cast(&brush)
}

/// Straight linear rgba, after decoding the colour channels.
pub fn to_rgba(color: Color) -> [f32; 4] {
    opencomp_core::rgba_from_srgb_bytes([color.a, color.r, color.g, color.b])
}

/// The inverse, for `ICompositionColorBrush.Color`.
pub fn from_rgba(color: [f32; 4]) -> Color {
    let [a, r, g, b] = opencomp_core::srgb_bytes_from_rgba(color);
    Color { a, r, g, b }
}
