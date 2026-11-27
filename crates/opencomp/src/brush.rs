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
use crate::object::prelude::*;
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

/// The colour channels are sRGB bytes and the renderer blends in linear light, so the
/// three that carry light are decoded; alpha is coverage and stays as it is.
pub fn to_rgba(color: Color) -> [f32; 4] {
    [
        opencomp_core::srgb(f32::from(color.r) / 255.0),
        opencomp_core::srgb(f32::from(color.g) / 255.0),
        opencomp_core::srgb(f32::from(color.b) / 255.0),
        f32::from(color.a) / 255.0,
    ]
}

pub fn from_rgba(color: [f32; 4]) -> Color {
    fn channel(value: f32) -> u8 {
        (opencomp_core::srgb_from_linear(value) * 255.0).round() as u8
    }
    Color {
        a: (color[3].clamp(0.0, 1.0) * 255.0).round() as u8,
        r: channel(color[0]),
        g: channel(color[1]),
        b: channel(color[2]),
    }
}
