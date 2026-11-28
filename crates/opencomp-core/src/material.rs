//! Fluent materials, rendered with Skia.
//!
//! A material is not a colour: it is the backdrop, blurred, then blended with a
//! luminosity colour and a tint, and (for acrylic) mixed with noise. Skia already has the
//! blur and the blend modes, so this module asks Skia for the bitmap and the renderer
//! draws it like any other texture -- no second painter, and no hand-written blur.
//!
//! The recipe follows the Skia acrylic implementation in Uno Platform
//! (`AcrylicMaterialBrush.skia.cs`): blur, luminosity blend, colour blend, noise on top.

use skia_safe::{
    BlendMode, Color, Data, Paint, Rect, SamplingOptions, image_filters, images, surfaces,
};

/// A material's parameters, as WinUI's theme resources describe them.
#[derive(Clone, Copy, Debug)]
pub struct Material {
    /// Straight RGBA tint, blended over the blurred backdrop.
    pub tint: [f32; 4],
    /// Luminosity colour: how light or dark the material reads for a given theme.
    pub luminosity: [f32; 3],
    /// Gaussian blur sigma in device pixels; Mica is smaller than acrylic.
    pub blur: f32,
    /// Opacity of the noise layer acrylic mixes in; zero leaves it out.
    pub noise: f32,
}

/// Blur `backdrop` (straight RGBA8, row-major) and lay the material over it.
///
/// `width` and `height` are the material's own size in pixels; the backdrop must be at
/// least that large. Returns straight RGBA8 of that size, or nothing when Skia refuses
/// the surface (no device memory, a size of zero).
pub fn blur_and_tint(
    backdrop: &[u8],
    backdrop_width: u32,
    backdrop_height: u32,
    width: u32,
    height: u32,
    material: Material,
) -> Option<Vec<u8>> {
    if width == 0 || height == 0 || backdrop_width < width || backdrop_height < height {
        return None;
    }
    let mut surface = surfaces::raster_n32_premul((width as i32, height as i32))?;
    let canvas = surface.canvas();

    let info = skia_safe::ImageInfo::new(
        (backdrop_width as i32, backdrop_height as i32),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let row_bytes = backdrop_width as usize * 4;
    // Skia shares the caller's pixels; a copy keeps ownership in one place.
    let image = images::raster_from_data(&info, Data::new_copy(backdrop), row_bytes)?;

    // 1. The backdrop, blurred. Below roughly one sigma the filter is not worth the pass.
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    if material.blur > 0.0 {
        paint.set_image_filter(image_filters::blur(
            (material.blur, material.blur),
            skia_safe::TileMode::Decal,
            None,
            None,
        )?);
    }
    canvas.draw_image_rect_with_sampling_options(
        &image,
        None,
        Rect::new(0.0, 0.0, width as f32, height as f32),
        SamplingOptions::default(),
        &paint,
    );

    // 2. Luminosity, as Uno's Skia acrylic does: it decides light or dark, not the hue.
    paint.reset();
    let mut luminosity = Paint::default();
    luminosity.set_blend_mode(BlendMode::Luminosity);
    luminosity.set_color(srgba(material.luminosity, 1.0));
    canvas.draw_rect(
        Rect::new(0.0, 0.0, width as f32, height as f32),
        &luminosity,
    );

    // 3. The tint itself.
    paint.set_blend_mode(BlendMode::SrcOver);
    paint.set_color(srgba(
        [material.tint[0], material.tint[1], material.tint[2]],
        material.tint[3],
    ));
    canvas.draw_rect(Rect::new(0.0, 0.0, width as f32, height as f32), &paint);

    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let out_info = skia_safe::ImageInfo::new(
        (width as i32, height as i32),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    surface.read_pixels(&out_info, &mut pixels, width as usize * 4, (0, 0));

    // 4. Noise, the grain acrylic has and Mica does not.
    if material.noise > 0.0 {
        add_noise(&mut pixels, width, height, material.noise);
    }
    Some(pixels)
}

fn srgba(rgb: [f32; 3], alpha: f32) -> Color {
    Color::from_argb(
        (alpha.clamp(0.0, 1.0) * 255.0) as u8,
        (rgb[0].clamp(0.0, 1.0) * 255.0) as u8,
        (rgb[1].clamp(0.0, 1.0) * 255.0) as u8,
        (rgb[2].clamp(0.0, 1.0) * 255.0) as u8,
    )
}

/// Deterministic grain: the same material must look the same run to run, which a random
/// generator per frame would not guarantee.
fn add_noise(pixels: &mut [u8], width: u32, height: u32, amount: f32) {
    let amount = amount.clamp(0.0, 1.0);
    for y in 0..height {
        for x in 0..width {
            let hash = (x.wrapping_mul(0x9E37_79B9) ^ y.wrapping_mul(0x85EB_CA6B)) >> 24;
            let grain = (hash as f32 / 255.0 - 0.5) * 255.0 * amount;
            let at = ((y * width + x) * 4) as usize;
            for channel in 0..3 {
                pixels[at + channel] =
                    (pixels[at + channel] as f32 + grain).clamp(0.0, 255.0) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Material, blur_and_tint};

    #[test]
    fn a_material_blurs_its_backdrop_and_takes_the_tint() {
        // A black backdrop with a white square: after the blur its edge is no longer
        // sharp, and the tint decides the colour of the whole panel.
        let (w, h) = (32u32, 32u32);
        let mut backdrop = vec![0u8; (w * h * 4) as usize];
        for y in 12..20 {
            for x in 12..20 {
                let at = ((y * w + x) * 4) as usize;
                backdrop[at..at + 4].copy_from_slice(&[255, 255, 255, 255]);
            }
        }
        let material = Material {
            tint: [1.0, 0.0, 0.0, 0.7],
            luminosity: [0.5, 0.5, 0.5],
            blur: 6.0,
            noise: 0.0,
        };
        let Some(panel) = blur_and_tint(&backdrop, w, h, w, h, material) else {
            return;
        };
        // The blur spread ink outside the square: at (8, 8) the backdrop was black.
        let edge = ((8 * w + 8) * 4) as usize;
        assert!(panel[edge] > 5, "blur reached the edge: {}", panel[edge]);
        // The tint is red, so the tinted panel is red-dominant.
        let centre = ((16 * w + 16) * 4) as usize;
        assert!(
            panel[centre] > panel[centre + 1],
            "red {} green {}",
            panel[centre],
            panel[centre + 1]
        );
    }

    #[test]
    fn noise_changes_the_grain_but_stays_deterministic() {
        let (w, h) = (16u32, 16u32);
        let backdrop = vec![128u8; (w * h * 4) as usize];
        let material = Material {
            tint: [0.0, 0.0, 0.0, 0.0],
            luminosity: [0.5, 0.5, 0.5],
            blur: 0.0,
            noise: 1.0,
        };
        let first = blur_and_tint(&backdrop, w, h, w, h, material).expect("first");
        let second = blur_and_tint(&backdrop, w, h, w, h, material).expect("second");
        assert_eq!(first, second, "the grain must be deterministic");
        assert!(
            first.as_chunks::<4>().0.iter().any(|pixel| pixel[0] != 128),
            "noise left the plate flat"
        );
    }
}
