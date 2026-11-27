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

/// A `Windows.UI.Color`-shaped `[a, r, g, b]` in sRGB to straight linear RGBA.
pub fn rgba_from_srgb_bytes([a, r, g, b]: [u8; 4]) -> [f32; 4] {
    [
        srgb(f32::from(r) / 255.0),
        srgb(f32::from(g) / 255.0),
        srgb(f32::from(b) / 255.0),
        f32::from(a) / 255.0,
    ]
}

/// The inverse, for a colour read back out through the ABI.
pub fn srgb_bytes_from_rgba([r, g, b, a]: [f32; 4]) -> [u8; 4] {
    fn channel(value: f32) -> u8 {
        (srgb_from_linear(value) * 255.0).round() as u8
    }
    [
        (a.clamp(0.0, 1.0) * 255.0).round() as u8,
        channel(r),
        channel(g),
        channel(b),
    ]
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
    fn a_colour_survives_the_round_trip() {
        for colour in [[255, 0, 0, 255], [18, 26, 31, 128], [0, 0, 0, 0]] {
            let linear = rgba_from_srgb_bytes(colour);
            assert_eq!(srgb_bytes_from_rgba(linear), colour);
        }
    }

    #[test]
    fn the_midpoint_byte_is_not_taken_for_the_midpoint_of_the_light() {
        // [a, r, g, b]: the colour channels are 128, which is 0.502 sRGB and 0.216 linear.
        let linear = rgba_from_srgb_bytes([255, 128, 128, 128]);
        assert!((linear[0] - 0.215_86).abs() < 1e-3);
        assert!((linear[3] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        assert!(close(srgb(-1.0), 0.0));
        assert!(close(srgb(2.0), 1.0));
    }
}
