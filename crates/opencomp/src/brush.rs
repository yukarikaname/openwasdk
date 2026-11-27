//! The brushes: `CompositionColorBrush` paints a flat colour, `CompositionSurfaceBrush` a bitmap.
//!
//! A colour brush's colour is copied into the visual when the brush is attached, and a surface
//! brush's surface is read the same way, so a `SetColor` or `SetSurface` after `SetBrush` does not
//! show up. That is the one visible shortcut here; real composition keeps a live link. The
//! placement hints on a surface brush are stored and answered, but the renderer stretches the
//! bitmap across the sprite and leaves filtering to the sampler.

use std::sync::Mutex;

use crate::bindings::Microsoft::UI::Composition::{
    CompositionBitmapInterpolationMode, CompositionBrush, CompositionColorBrush,
    CompositionStretch, CompositionSurfaceBrush, ICompositionBrush, ICompositionBrush_Impl,
    ICompositionColorBrush, ICompositionColorBrush_Impl, ICompositionObject, ICompositionObject2,
    ICompositionObject4, ICompositionObject5, ICompositionSurface, ICompositionSurfaceBrush,
    ICompositionSurfaceBrush_Impl,
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

/// What a surface brush keeps: the surface and the placement hints around it.
struct SurfaceState {
    surface: Option<ICompositionSurface>,
    interpolation: CompositionBitmapInterpolationMode,
    stretch: CompositionStretch,
    horizontal: f32,
    vertical: f32,
}

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionBrush,
    ICompositionSurfaceBrush
)]
pub struct SurfaceBrush {
    state: Mutex<SurfaceState>,
}

composition_object!(SurfaceBrush_Impl);

impl ICompositionBrush_Impl for SurfaceBrush_Impl {}

impl ICompositionSurfaceBrush_Impl for SurfaceBrush_Impl {
    fn BitmapInterpolationMode(&self) -> Result<CompositionBitmapInterpolationMode> {
        Ok(self.state.lock().unwrap().interpolation)
    }

    fn SetBitmapInterpolationMode(&self, value: CompositionBitmapInterpolationMode) -> Result<()> {
        self.state.lock().unwrap().interpolation = value;
        Ok(())
    }

    fn HorizontalAlignmentRatio(&self) -> Result<f32> {
        Ok(self.state.lock().unwrap().horizontal)
    }

    fn SetHorizontalAlignmentRatio(&self, value: f32) -> Result<()> {
        self.state.lock().unwrap().horizontal = value;
        Ok(())
    }

    fn Stretch(&self) -> Result<CompositionStretch> {
        Ok(self.state.lock().unwrap().stretch)
    }

    fn SetStretch(&self, value: CompositionStretch) -> Result<()> {
        self.state.lock().unwrap().stretch = value;
        Ok(())
    }

    fn Surface(&self) -> Result<ICompositionSurface> {
        match self.state.lock().unwrap().surface.clone() {
            Some(surface) => Ok(surface),
            // The property is nullable in the ABI; the trait cannot say so.
            None => crate::object::not_implemented(),
        }
    }

    fn SetSurface(&self, value: windows_core::Ref<ICompositionSurface>) -> Result<()> {
        self.state.lock().unwrap().surface = value.cloned();
        Ok(())
    }

    fn VerticalAlignmentRatio(&self) -> Result<f32> {
        Ok(self.state.lock().unwrap().vertical)
    }

    fn SetVerticalAlignmentRatio(&self, value: f32) -> Result<()> {
        self.state.lock().unwrap().vertical = value;
        Ok(())
    }
}

/// A surface brush with no surface yet; a `SetSurface` fills it in.
pub fn surface_brush() -> Result<CompositionSurfaceBrush> {
    surface_brush_with(None)
}

/// A surface brush that paints `surface` once it is attached to a sprite.
pub fn surface_brush_with(surface: Option<ICompositionSurface>) -> Result<CompositionSurfaceBrush> {
    let brush: ICompositionSurfaceBrush = SurfaceBrush {
        state: Mutex::new(SurfaceState {
            surface,
            interpolation: CompositionBitmapInterpolationMode::Linear,
            stretch: CompositionStretch::Fill,
            horizontal: 0.0,
            vertical: 0.0,
        }),
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

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::bindings::Microsoft::UI::Composition::{ICompositor, ISpriteVisual};
    use windows_core::Interface;

    fn compositor() -> ICompositor {
        crate::compositor::instance().unwrap().cast().unwrap()
    }

    #[test]
    fn a_surface_brush_puts_the_bitmap_on_the_sprite() {
        let compositor = compositor();
        let drawing = crate::surface::create(2, 2).unwrap();
        let surface: ICompositionSurface = drawing.cast().unwrap();
        let brush = compositor.CreateSurfaceBrushWithSurface(&surface).unwrap();
        let brush: CompositionBrush = brush.cast().unwrap();

        let sprite = compositor.CreateSpriteVisual().unwrap();
        let sprite_visual: ISpriteVisual = sprite.cast().unwrap();
        sprite_visual.SetBrush(&brush).unwrap();

        let node = crate::node::resolve(&sprite).unwrap();
        let (id, width, height, pixels) = crate::surface::surface_image(&node).unwrap();
        assert_eq!((width, height), (2, 2));
        assert_eq!(pixels, vec![0; 16]);

        // An unsized sprite takes the surface's size, and the bitmap is tinted white.
        let tree = crate::node::snapshot(&node).unwrap();
        assert_eq!(tree.image, Some(id));
        assert_eq!(tree.size, [2.0, 2.0]);
        assert_eq!(tree.color, [1.0; 4]);
    }

    #[test]
    fn a_colour_brush_after_a_surface_brush_paints_the_colour() {
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        let sprite_visual: ISpriteVisual = sprite.cast().unwrap();

        let drawing = crate::surface::create(1, 1).unwrap();
        let surface: ICompositionSurface = drawing.cast().unwrap();
        let surface_brush: CompositionBrush = compositor
            .CreateSurfaceBrushWithSurface(&surface)
            .unwrap()
            .cast()
            .unwrap();
        sprite_visual.SetBrush(&surface_brush).unwrap();

        let color_brush: CompositionBrush = compositor
            .CreateColorBrushWithColor(Color {
                a: 255,
                r: 255,
                g: 0,
                b: 0,
            })
            .unwrap()
            .cast()
            .unwrap();
        sprite_visual.SetBrush(&color_brush).unwrap();

        let node = crate::node::resolve(&sprite).unwrap();
        assert!(crate::surface::surface_image(&node).is_none());
        let tree = crate::node::snapshot(&node).unwrap();
        assert_eq!(tree.image, None);
        assert_eq!(tree.color, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn a_surface_brush_keeps_its_settings() {
        let compositor = compositor();
        let brush = compositor.CreateSurfaceBrush().unwrap();
        let surface_brush: ICompositionSurfaceBrush = brush.cast().unwrap();
        // Unset, the property is nullable in the ABI and the trait cannot say so.
        assert!(surface_brush.Surface().is_err());

        let drawing = crate::surface::create(1, 1).unwrap();
        let surface: ICompositionSurface = drawing.cast().unwrap();
        surface_brush.SetSurface(&surface).unwrap();
        assert!(surface_brush.Surface().is_ok());

        surface_brush
            .SetStretch(CompositionStretch::Uniform)
            .unwrap();
        assert_eq!(
            surface_brush.Stretch().unwrap(),
            CompositionStretch::Uniform
        );
        surface_brush
            .SetBitmapInterpolationMode(CompositionBitmapInterpolationMode::NearestNeighbor)
            .unwrap();
        assert_eq!(
            surface_brush.BitmapInterpolationMode().unwrap(),
            CompositionBitmapInterpolationMode::NearestNeighbor
        );
    }
}
