//! The portable half of the compositor: the tree, the animation sampler and the draw list.
//!
//! `opencomp` is the Windows ABI shell around this. Everything here is GPU-agnostic
//! except the `preview` feature, which adds a wgpu renderer and a host for looking at
//! a tree on the machine you are sitting in front of. The DLL does not use it: on
//! Windows the compositor is D3D11, the same device the client hands over.

mod animation;
mod color;
mod texture_id;
mod tree;

#[cfg(feature = "preview")]
mod host;
#[cfg(feature = "preview")]
mod render;
#[cfg(feature = "preview")]
mod texture;

pub use animation::{Easing, Frame, Keyframes};
pub use color::{rgba_from_srgb_bytes, srgb, srgb_bytes_from_rgba, srgb_from_linear};
pub use texture_id::{TextureId, WHITE};
pub use tree::{Draw, Rect, Visual};

#[cfg(feature = "preview")]
pub use host::Host;
#[cfg(feature = "preview")]
pub use render::Renderer;

impl Visual {
    /// Flatten this tree into the units a backend draws, in paint order.
    pub fn draws(&self) -> Vec<Draw> {
        let mut out = Vec::new();
        tree::flatten_root(self, &mut out);
        out
    }
}

#[cfg(all(test, feature = "preview"))]
mod tests {
    use super::*;

    fn pixel(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * width + x) * 4) as usize;
        pixels[at..at + 4].try_into().unwrap()
    }

    /// GPU rounding is allowed to differ from the CPU's by a byte.
    fn close(actual: [u8; 4], expected: [u8; 4]) -> bool {
        actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.abs_diff(expected) <= 2)
    }

    /// A 2x2 sRGB bitmap: red and green on the top row, blue and white on the bottom.
    fn quadrants() -> [u8; 16] {
        [
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
        ]
    }

    #[test]
    fn a_visual_draws_over_the_background() {
        let Some(mut renderer) = Renderer::headless((16, 16), [0.0, 0.0, 1.0, 1.0]) else {
            // No adapter on this machine; the test cannot say anything either way.
            return;
        };
        renderer.set_root(Visual::rect(8.0, 8.0, [1.0, 0.0, 0.0, 1.0]).at(4.0, 4.0));
        let pixels = renderer.render_to_pixels();
        assert_eq!(pixel(&pixels, 16, 8, 8), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 16, 0, 0), [0, 0, 255, 255]);
    }

    #[test]
    fn a_child_is_placed_relative_to_its_parent() {
        let Some(mut renderer) = Renderer::headless((16, 16), [0.0, 0.0, 0.0, 1.0]) else {
            return;
        };
        let mut parent = Visual::rect(12.0, 12.0, [0.0, 0.0, 0.0, 0.0]).at(2.0, 2.0);
        parent.push(Visual::rect(4.0, 4.0, [0.0, 1.0, 0.0, 1.0]).at(2.0, 2.0));
        renderer.set_root(parent);
        let pixels = renderer.render_to_pixels();
        assert_eq!(pixel(&pixels, 16, 5, 5), [0, 255, 0, 255]);
        assert_eq!(pixel(&pixels, 16, 2, 2), [0, 0, 0, 255]);
    }

    #[test]
    fn children_are_clipped_to_the_parent() {
        let Some(mut renderer) = Renderer::headless((16, 16), [0.0, 0.0, 1.0, 1.0]) else {
            return;
        };
        let mut parent = Visual::rect(4.0, 4.0, [0.0, 0.0, 0.0, 0.0])
            .at(4.0, 4.0)
            .clip_to(0.0, 0.0, 4.0, 4.0);
        parent.push(Visual::rect(8.0, 8.0, [1.0, 0.0, 0.0, 1.0]));
        renderer.set_root(parent);
        let pixels = renderer.render_to_pixels();
        assert_eq!(pixel(&pixels, 16, 5, 5), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 16, 10, 10), [0, 0, 255, 255]);
    }

    #[test]
    fn a_zero_opacity_visual_is_skipped() {
        let Some(mut renderer) = Renderer::headless((8, 8), [0.0, 0.0, 1.0, 1.0]) else {
            return;
        };
        renderer.set_root(
            Visual::rect(8.0, 8.0, [1.0, 0.0, 0.0, 1.0])
                .at(0.0, 0.0)
                .with_opacity(0.0),
        );
        let pixels = renderer.render_to_pixels();
        assert_eq!(pixel(&pixels, 8, 4, 4), [0, 0, 255, 255]);
    }

    #[test]
    fn a_scale_factor_doubles_the_drawn_rectangle() {
        let Some(mut renderer) = Renderer::headless((16, 16), [0.0, 0.0, 1.0, 1.0]) else {
            return;
        };
        renderer.set_scale(2.0);
        renderer.set_root(Visual::rect(4.0, 4.0, [1.0, 0.0, 0.0, 1.0]));
        let pixels = renderer.render_to_pixels();
        assert_eq!(pixel(&pixels, 16, 7, 7), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 16, 8, 8), [0, 0, 255, 255]);
    }

    #[test]
    fn a_visual_outside_the_target_is_skipped() {
        let Some(mut renderer) = Renderer::headless((8, 8), [0.0, 0.0, 0.0, 1.0]) else {
            return;
        };
        renderer.set_root(Visual::rect(4.0, 4.0, [1.0, 1.0, 1.0, 1.0]).at(20.0, 20.0));
        let pixels = renderer.render_to_pixels();
        assert_eq!(pixel(&pixels, 8, 0, 0), [0, 0, 0, 255]);
    }

    #[test]
    fn a_visual_scaled_by_two_covers_the_doubled_rectangle() {
        let Some(mut renderer) = Renderer::headless((16, 16), [0.0, 0.0, 1.0, 1.0]) else {
            return;
        };
        let mut visual = Visual::rect(4.0, 4.0, [1.0, 0.0, 0.0, 1.0]).at(2.0, 2.0);
        visual.scale = [2.0, 2.0];
        renderer.set_root(visual);
        let pixels = renderer.render_to_pixels();
        // A 4x4 rectangle at (2, 2) grows to 8x8: its far corner is now red, the place just
        // outside the old one still shows the background.
        assert_eq!(pixel(&pixels, 16, 9, 9), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 16, 10, 10), [0, 0, 255, 255]);
        assert_eq!(pixel(&pixels, 16, 1, 1), [0, 0, 255, 255]);
    }

    #[test]
    fn a_visual_rotated_by_45_degrees_leaves_its_old_corners_empty() {
        let Some(mut renderer) = Renderer::headless((16, 16), [0.0, 0.0, 1.0, 1.0]) else {
            return;
        };
        let mut visual = Visual::rect(8.0, 8.0, [1.0, 0.0, 0.0, 1.0]).at(4.0, 4.0);
        visual.rotation = std::f32::consts::FRAC_PI_4;
        visual.center = [4.0, 4.0];
        renderer.set_root(visual);
        let pixels = renderer.render_to_pixels();
        // The square turns into a diamond around (8, 8): the centre stays covered, while each
        // corner of the old square falls outside the rotated quad.
        assert_eq!(pixel(&pixels, 16, 8, 8), [255, 0, 0, 255]);
        for (x, y) in [(4, 4), (12, 4), (12, 12), (4, 12)] {
            assert_eq!(
                pixel(&pixels, 16, x, y),
                [0, 0, 255, 255],
                "corner {x},{y} should be background"
            );
        }
    }

    #[test]
    fn a_child_of_a_scaled_parent_is_placed_in_the_scaled_space() {
        let Some(mut renderer) = Renderer::headless((16, 16), [0.0, 0.0, 1.0, 1.0]) else {
            return;
        };
        let mut parent = Visual::rect(8.0, 8.0, [0.0, 0.0, 0.0, 0.0]).at(2.0, 2.0);
        parent.scale = [2.0, 2.0];
        parent.push(Visual::rect(2.0, 2.0, [0.0, 1.0, 0.0, 1.0]).at(1.0, 1.0));
        renderer.set_root(parent);
        let pixels = renderer.render_to_pixels();
        // The child's (1, 1) is two parent units from the canvas origin, so it lands at (4, 4)
        // and its 2x2 rectangle covers four screen pixels in each direction.
        assert_eq!(pixel(&pixels, 16, 4, 4), [0, 255, 0, 255]);
        assert_eq!(pixel(&pixels, 16, 7, 7), [0, 255, 0, 255]);
        assert_eq!(pixel(&pixels, 16, 8, 8), [0, 0, 255, 255]);
        assert_eq!(pixel(&pixels, 16, 3, 3), [0, 0, 255, 255]);
    }

    #[test]
    fn the_device_scale_applies_after_the_visual_scale() {
        let Some(mut renderer) = Renderer::headless((16, 16), [0.0, 0.0, 1.0, 1.0]) else {
            return;
        };
        renderer.set_scale(2.0);
        let mut visual = Visual::rect(2.0, 2.0, [1.0, 0.0, 0.0, 1.0]);
        visual.scale = [2.0, 2.0];
        renderer.set_root(visual);
        let pixels = renderer.render_to_pixels();
        // Two composition units scaled by two are four units, eight device pixels at 2x.
        assert_eq!(pixel(&pixels, 16, 7, 7), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 16, 8, 8), [0, 0, 255, 255]);
    }

    #[test]
    fn a_textured_visual_draws_the_quadrants_of_its_bitmap() {
        let Some(mut renderer) = Renderer::headless((8, 8), [0.0, 0.0, 0.0, 1.0]) else {
            return;
        };
        renderer.upload_rgba(1, 2, 2, &quadrants());
        renderer.set_root(Visual::rect(8.0, 8.0, [1.0; 4]).with_image(1));
        let pixels = renderer.render_to_pixels();
        // Sample away from the centre: linear filtering blends where the quadrants meet.
        assert_eq!(pixel(&pixels, 8, 1, 1), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 8, 6, 1), [0, 255, 0, 255]);
        assert_eq!(pixel(&pixels, 8, 1, 6), [0, 0, 255, 255]);
        assert_eq!(pixel(&pixels, 8, 6, 6), [255, 255, 255, 255]);
    }

    #[test]
    fn a_uv_rectangle_selects_part_of_the_bitmap() {
        let Some(mut renderer) = Renderer::headless((8, 8), [0.0, 0.0, 0.0, 1.0]) else {
            return;
        };
        renderer.upload_rgba(1, 2, 2, &quadrants());
        // The top half of the bitmap is red and green; the bottom row must never be sampled.
        renderer.set_root(
            Visual::rect(8.0, 8.0, [1.0; 4])
                .with_image(1)
                .with_uv([0.0, 0.0, 1.0, 0.5]),
        );
        let pixels = renderer.render_to_pixels();
        assert_eq!(pixel(&pixels, 8, 1, 1), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 8, 6, 1), [0, 255, 0, 255]);
        assert_eq!(pixel(&pixels, 8, 1, 3), [255, 0, 0, 255]);
        assert_eq!(pixel(&pixels, 8, 6, 3), [0, 255, 0, 255]);
    }

    #[test]
    fn the_visual_colour_multiplies_the_texel() {
        let Some(mut renderer) = Renderer::headless((8, 8), [0.0, 0.0, 0.0, 1.0]) else {
            return;
        };
        renderer.upload_rgba(1, 1, 1, &[255, 255, 255, 255]);
        // White texels under a half-red colour are half the light, 188 in sRGB and not 128.
        let [a, r, g, b] = srgb_bytes_from_rgba([0.5, 0.0, 0.0, 1.0]);
        renderer.set_root(Visual::rect(8.0, 8.0, [0.5, 0.0, 0.0, 1.0]).with_image(1));
        let pixels = renderer.render_to_pixels();
        let actual = pixel(&pixels, 8, 4, 4);
        assert!(
            close(actual, [r, g, b, a]),
            "half red came out as {actual:?}"
        );
    }

    #[test]
    fn an_untextured_visual_draws_its_flat_colour() {
        let Some(mut renderer) = Renderer::headless((8, 8), [0.0, 0.0, 0.0, 1.0]) else {
            return;
        };
        renderer.set_root(Visual::rect(8.0, 8.0, [0.0, 0.0, 1.0, 1.0]));
        let pixels = renderer.render_to_pixels();
        // No image means the built-in white texture, so only the colour is left.
        assert_eq!(pixel(&pixels, 8, 4, 4), [0, 0, 255, 255]);
    }

    #[test]
    fn a_released_bitmap_falls_back_to_the_flat_colour() {
        let Some(mut renderer) = Renderer::headless((8, 8), [0.0, 0.0, 0.0, 1.0]) else {
            return;
        };
        renderer.upload_rgba(1, 2, 2, &quadrants());
        renderer.release(1);
        renderer.set_root(Visual::rect(8.0, 8.0, [0.0, 1.0, 0.0, 1.0]).with_image(1));
        let pixels = renderer.render_to_pixels();
        assert_eq!(pixel(&pixels, 8, 4, 4), [0, 255, 0, 255]);
    }

    #[test]
    fn a_rounded_visual_clears_its_corners() {
        let Some(mut renderer) = Renderer::headless((16, 16), [0.0, 0.0, 1.0, 1.0]) else {
            return;
        };
        renderer.set_root(Visual::rect(16.0, 16.0, [1.0, 0.0, 0.0, 1.0]).rounded(8.0));
        let pixels = renderer.render_to_pixels();
        // A radius of 8 on a 16x16 square is a circle about its centre: the corner pixel lies
        // outside it, the middle well inside.
        assert_eq!(pixel(&pixels, 16, 0, 0), [0, 0, 255, 255]);
        assert_eq!(pixel(&pixels, 16, 8, 8), [255, 0, 0, 255]);
    }

    #[test]
    fn a_zero_radius_still_fills_the_corners() {
        let Some(mut renderer) = Renderer::headless((16, 16), [0.0, 0.0, 1.0, 1.0]) else {
            return;
        };
        renderer.set_root(Visual::rect(16.0, 16.0, [1.0, 0.0, 0.0, 1.0]).rounded(0.0));
        let pixels = renderer.render_to_pixels();
        // The shader must not soften a corner no radius asked for.
        for (x, y) in [(0, 0), (15, 0), (0, 15), (15, 15), (8, 8)] {
            assert_eq!(
                pixel(&pixels, 16, x, y),
                [255, 0, 0, 255],
                "pixel {x},{y} should be fill"
            );
        }
    }

    #[test]
    fn a_rounded_visual_keeps_the_texel_inside_the_shape() {
        let Some(mut renderer) = Renderer::headless((16, 16), [1.0, 0.0, 1.0, 1.0]) else {
            return;
        };
        renderer.upload_rgba(1, 1, 1, &[0, 255, 0, 255]);
        renderer.set_root(
            Visual::rect(16.0, 16.0, [1.0; 4])
                .with_image(1)
                .rounded(8.0),
        );
        let pixels = renderer.render_to_pixels();
        // The magenta background shows through the cut corner; inside the circle the green
        // texel is sampled and, under a white colour, comes out unchanged.
        assert_eq!(pixel(&pixels, 16, 0, 0), [255, 0, 255, 255]);
        assert_eq!(pixel(&pixels, 16, 5, 5), [0, 255, 0, 255]);
        assert_eq!(pixel(&pixels, 16, 8, 8), [0, 255, 0, 255]);
    }

    #[test]
    fn a_rotated_rounded_visual_keeps_its_corners_round() {
        let Some(mut renderer) = Renderer::headless((32, 32), [0.0, 0.0, 1.0, 1.0]) else {
            return;
        };
        let mut visual = Visual::rect(16.0, 16.0, [1.0, 0.0, 0.0, 1.0])
            .at(8.0, 8.0)
            .rounded(8.0);
        visual.rotation = std::f32::consts::FRAC_PI_4;
        visual.center = [8.0, 8.0];
        renderer.set_root(visual);
        let pixels = renderer.render_to_pixels();
        // The square turns about (16, 16), so its local corner (1, 1) lands near (16, 6),
        // inside the rotated quad but outside the circle of radius 8 that corner draws: only
        // measuring the distance in local space leaves that pixel background.
        assert_eq!(pixel(&pixels, 32, 16, 6), [0, 0, 255, 255]);
        assert_eq!(pixel(&pixels, 32, 16, 16), [255, 0, 0, 255]);
    }
}
