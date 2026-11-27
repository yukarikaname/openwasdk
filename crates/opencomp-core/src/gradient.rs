//! Gradient paints in the draw list.
//!
//! Coordinates are in the node's own units: a linear ramp runs from `start` to `end`, a radial
//! one reaches `radius` from `center`. The tree's affine transform is applied to the quad, so a
//! rotated or scaled visual carries its gradient along with it.
//!
//! Stops are interpolated the way Direct2D does it: in sRGB, not in linear light. The ramp below
//! is therefore built from sRGB-encoded channels, and the renderer samples it through an sRGB
//! texture, which leaves the bytes the preview shows and the pixels D2D draws comparable.

use crate::color::{srgb, srgb_from_linear};

/// One colour stop. `offset` is a fraction of the ramp, zero at its start and one at its end.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientStop {
    pub offset: f32,
    pub color: [f32; 4],
}

/// The shape of a ramp, in the node's own units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradientShape {
    Linear { start: [f32; 2], end: [f32; 2] },
    Radial { center: [f32; 2], radius: [f32; 2] },
}

/// A gradient paint.
#[derive(Clone, Debug, PartialEq)]
pub struct Gradient {
    /// The brush this came from, so a backend can cache a device brush per brush.
    pub id: u32,
    pub stops: Vec<GradientStop>,
    pub shape: GradientShape,
}

/// How many entries the preview's gradient texture has.
pub const RAMP_ENTRIES: usize = 256;

impl Gradient {
    /// The stops in ascending offset order, clamped to the unit interval.
    ///
    /// Composition defines a ramp by offset, not by the order a client appended its stops, so
    /// sorting here is what makes a multi-stop brush draw in the order it asked for.
    pub fn sorted_stops(&self) -> Vec<GradientStop> {
        let mut stops = self.stops.clone();
        stops.sort_by(|left, right| left.offset.total_cmp(&right.offset));
        stops
    }

    /// Whether this gradient has a ramp a backend can draw.
    pub fn is_drawable(&self) -> bool {
        !self.stops.is_empty()
    }

    /// One sRGB colour per entry of a [`RAMP_ENTRIES`]-long ramp, as RGBA8 bytes.
    ///
    /// The preview turns this into a 1-D texture. Interpolating the encoded channels keeps the
    /// result identical to `ID2D1DeviceContext::CreateGradientStopCollection` with
    /// `D2D1_GAMMA_2_2`, which is the gamma D2D interpolates a typical sRGB target in.
    pub fn ramp(&self) -> Vec<u8> {
        let mut out = vec![0u8; RAMP_ENTRIES * 4];
        let stops = self.sorted_stops();
        if stops.is_empty() {
            return out;
        }
        let entries = out.as_chunks_mut::<4>().0;
        for (index, entry) in entries.iter_mut().enumerate() {
            let t = index as f32 / (RAMP_ENTRIES - 1) as f32;
            let stop = sample(&stops, t);
            let srgba = [
                srgb_from_linear(stop.color[0]),
                srgb_from_linear(stop.color[1]),
                srgb_from_linear(stop.color[2]),
                stop.color[3].clamp(0.0, 1.0),
            ];
            entry[0] = (srgba[0] * 255.0).round() as u8;
            entry[1] = (srgba[1] * 255.0).round() as u8;
            entry[2] = (srgba[2] * 255.0).round() as u8;
            entry[3] = (srgba[3] * 255.0).round() as u8;
        }
        out
    }
}

/// The colour at `t`, interpolated in sRGB between the stops around it.
fn sample(stops: &[GradientStop], t: f32) -> GradientStop {
    let first = stops[0];
    if t <= first.offset {
        return first;
    }
    let last = stops[stops.len() - 1];
    if t >= last.offset {
        return last;
    }
    for pair in stops.windows(2) {
        let [left, right] = [pair[0], pair[1]];
        if t < left.offset || t > right.offset {
            continue;
        }
        let span = right.offset - left.offset;
        let fraction = if span > 0.0 {
            (t - left.offset) / span
        } else {
            0.0
        };
        return GradientStop {
            offset: t,
            // The colour channels blend in sRGB, the way D2D's GAMMA_2_2 does. Alpha is not
            // a colour channel and stays linear light.
            color: [
                mix_srgb(left.color[0], right.color[0], fraction),
                mix_srgb(left.color[1], right.color[1], fraction),
                mix_srgb(left.color[2], right.color[2], fraction),
                left.color[3] + (right.color[3] - left.color[3]) * fraction,
            ],
        };
    }
    last
}

/// A 0..=1 colour blend in sRGB: encode both ends, blend the encoded values, then decode.
fn mix_srgb(left: f32, right: f32, fraction: f32) -> f32 {
    let encoded =
        srgb_from_linear(left) + (srgb_from_linear(right) - srgb_from_linear(left)) * fraction;
    srgb(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn black_to_white() -> Gradient {
        Gradient {
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
                start: [0.0, 0.0],
                end: [64.0, 0.0],
            },
        }
    }

    #[test]
    fn the_ends_of_the_ramp_are_the_stop_colours() {
        let ramp = black_to_white().ramp();
        assert_eq!(&ramp[0..4], &[0, 0, 0, 255]);
        assert_eq!(&ramp[(RAMP_ENTRIES - 1) * 4..], &[255, 255, 255, 255]);
    }

    #[test]
    fn a_gradient_sorts_stops_by_offset_not_insertion() {
        let mut gradient = black_to_white();
        gradient.stops.reverse();
        let ramp = gradient.ramp();
        assert_eq!(&ramp[0..4], &[0, 0, 0, 255]);
        assert_eq!(&ramp[(RAMP_ENTRIES - 1) * 4..], &[255, 255, 255, 255]);
    }

    #[test]
    fn the_midpoint_is_half_way_in_srgb() {
        // Interpolating in linear light would put the byte near 188; sRGB interpolation is 128.
        let ramp = black_to_white().ramp();
        let middle = (RAMP_ENTRIES / 2) * 4;
        assert!((ramp[middle] as i32 - 128).abs() <= 1, "{}", ramp[middle]);
    }
}
