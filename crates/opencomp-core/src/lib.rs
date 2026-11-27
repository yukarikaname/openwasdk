//! The portable half of the compositor: a visual tree and a wgpu renderer.
//!
//! `opencomp` is the Windows ABI shell around this; the core has no WinRT in it, so it builds and
//! tests on every host. That is what makes a macOS backend possible: the renderer talks to Metal
//! through wgpu the same way it talks to Vulkan or D3D12 elsewhere.

mod render;
mod tree;

pub use render::Renderer;
pub use tree::{Rect, Visual};

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(pixels: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let at = ((y * width + x) * 4) as usize;
        pixels[at..at + 4].try_into().unwrap()
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
}
