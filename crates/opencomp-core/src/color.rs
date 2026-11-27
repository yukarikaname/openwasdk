//! The one colour conversion the renderer needs.
//!
//! A surface in an `*Srgb` format stores sRGB bytes and blends in linear light, so a
//! colour handed to the shader has to be linear before it gets there: a value of
//! 0.5 written straight into an sRGB target comes out as byte 188, not 128.

/// Decode one sRGB channel, 0..=1, to linear light.
pub fn srgb(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// Encode linear light back to sRGB, 0..=1.
pub fn srgb_from_linear(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 1e-4
    }

    #[test]
    fn the_ends_are_fixed_points() {
        assert!(close(srgb(0.0), 0.0));
        assert!(close(srgb(1.0), 1.0));
        assert!(close(srgb_from_linear(0.0), 0.0));
        assert!(close(srgb_from_linear(1.0), 1.0));
    }

    #[test]
    fn the_midpoint_is_a_quarter_of_the_light() {
        // The value that should have made this obvious: 0.5 sRGB is 0.214 linear, so a
        // shader that skips the decode writes 188 where the client asked for 128.
        assert!(close(srgb(0.5), 0.214_04));
        assert!(close(srgb_from_linear(0.214_04), 0.5));
    }

    #[test]
    fn the_two_directions_undo_each_other() {
        for step in 0..=20 {
            let value = step as f32 / 20.0;
            assert!(close(srgb_from_linear(srgb(value)), value));
        }
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        assert!(close(srgb(-1.0), 0.0));
        assert!(close(srgb(2.0), 1.0));
    }
}
