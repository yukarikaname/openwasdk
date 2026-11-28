//! Text metrics: the layout layer asks, the host answers.
//!
//! Shaping lives outside this crate -- on the preview it is parley, on Windows it is the
//! system text stack -- so a measurement is installed rather than compiled in. Without one
//! a line is approximated at half its font size per character.

use std::sync::OnceLock;

/// Measures a string at a font size: width and height, in the element's units.
pub type Metrics = fn(&str, f32) -> Option<(f32, f32)>;

static METRICS: OnceLock<Metrics> = OnceLock::new();

/// Install the measurement the layout pass should use. The first call wins: a process has
/// one text stack, and a page is measured against the same one it is drawn with.
pub fn set_metrics(metrics: Metrics) {
    let _ = METRICS.set(metrics);
}

pub(crate) fn measure(text: &str, font_size: f32) -> Option<(f32, f32)> {
    METRICS
        .get()
        .copied()
        .and_then(|metrics| metrics(text, font_size))
}
