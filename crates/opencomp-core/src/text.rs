//! Glyph shaping and rasterisation for the preview renderer.
//!
//! A `TextBlock` reaches the compositor as a string and a font size. Parley does the work
//! a text stack needs doing -- font fallback, shaping, bidi, line breaking -- and this
//! module only rasterises the glyphs it returns, with ab_glyph, into an RGBA bitmap the
//! renderer draws like any other texture. Results are cached by (text, size) because a
//! preview re-renders the same strings every frame.

use std::collections::HashMap;

use ab_glyph::{Font, PxScale, point};
use parley::{FontContext, LayoutContext};

/// One rasterised run: straight RGBA8, row-major, with its natural size.
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

struct Engine {
    fonts: FontContext,
    layouts: LayoutContext<()>,
    cache: HashMap<(String, u32), Raster>,
}

static ENGINE: std::sync::OnceLock<std::sync::Mutex<Option<Engine>>> = std::sync::OnceLock::new();

fn engine() -> &'static std::sync::Mutex<Option<Engine>> {
    ENGINE.get_or_init(|| {
        std::sync::Mutex::new(Some(Engine {
            fonts: FontContext::new(),
            layouts: LayoutContext::new(),
            cache: HashMap::new(),
        }))
    })
}

impl Engine {
    /// Shape and rasterise one string. Sizes come back in the string's own scale.
    fn rasterize(&mut self, text: &str, px: f32, color: [f32; 4]) -> Raster {
        let size = px.max(1.0);
        let mut builder = self
            .layouts
            .ranged_builder(&mut self.fonts, text, 1.0, true);
        builder.push_default(parley::StyleProperty::FontSize(size));
        let mut layout = builder.build(text);
        layout.break_all_lines(None);

        let trace = std::env::var_os("OPENCOMP_TEXT_TRACE").is_some();
        let mut drawn = 0usize;
        let mut failed_fonts = 0usize;
        let width = layout.width().ceil().max(1.0) as u32;
        let height = layout.height().ceil().max(1.0) as u32;
        let mut pixels = vec![0u8; (width * height * 4) as usize];

        for line in layout.lines() {
            for item in line.items() {
                let parley::PositionedLayoutItem::GlyphRun(run) = item else {
                    continue;
                };
                let data = run.run().font();
                let Ok(font) =
                    ab_glyph::FontRef::try_from_slice_and_index(data.data.as_ref(), data.index)
                else {
                    failed_fonts += 1;
                    continue;
                };
                let scale = PxScale::from(size);
                for glyph in run.positioned_glyphs() {
                    let outline = font.outline_glyph(ab_glyph::Glyph {
                        id: ab_glyph::GlyphId(glyph.id as u16),
                        scale,
                        position: point(glyph.x, glyph.y),
                    });
                    let Some(outline) = outline else {
                        continue;
                    };
                    let bounds = outline.px_bounds();
                    drawn += 1;
                    outline.draw(|gx, gy, coverage| {
                        let x = bounds.min.x as i32 + gx as i32;
                        let y = bounds.min.y as i32 + gy as i32;
                        blend(&mut pixels, width, height, x, y, coverage, color);
                    });
                }
            }
        }

        if trace {
            eprintln!(
                "text: {text:?} size={size} layout={}x{} raster={width}x{height} lines={} drawn={drawn} nofont={failed_fonts}",
                layout.width(),
                layout.height(),
                layout.lines().count()
            );
        }
        Raster {
            width,
            height,
            pixels,
        }
    }
}

/// Composite one glyph pixel into the run, source-over, premultiplied by coverage.
fn blend(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    coverage: f32,
    color: [f32; 4],
) {
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return;
    }
    let offset = ((y as u32 * width + x as u32) * 4) as usize;
    let alpha = (coverage * color[3]).clamp(0.0, 1.0);
    let existing = pixels[offset + 3] as f32 / 255.0;
    let combined = alpha + existing * (1.0 - alpha);
    if combined <= 0.0 {
        return;
    }
    for channel in 0..3 {
        let old = pixels[offset + channel] as f32 / 255.0;
        let new = color[channel];
        pixels[offset + channel] = ((old * existing * (1.0 - alpha) + new * alpha) / combined
            * 255.0)
            .clamp(0.0, 255.0) as u8;
    }
    pixels[offset + 3] = (combined * 255.0) as u8;
}

/// Measured sizes, keyed by (text, font size bits).
type Measurements = HashMap<(String, u32), (f32, f32)>;

/// The size `text` occupies at `px`, from the same shaping stack that rasterises it.
///
/// Layout asks this for every string; the answer is cached by (text, size) because a page
/// repeats its strings on every pass. `None` means the font stack could not be built, and
/// the caller keeps its own approximation.
pub fn measure(text: &str, px: f32) -> Option<(f32, f32)> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Measurements>> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let size = px.max(1.0);
    let key = (text.to_string(), size.to_bits());
    if let Ok(map) = cache.lock()
        && let Some(measured) = map.get(&key)
    {
        return Some(*measured);
    }
    let mut guard = engine().lock().ok()?;
    let engine = guard.as_mut()?;
    let mut builder = engine
        .layouts
        .ranged_builder(&mut engine.fonts, text, 1.0, true);
    builder.push_default(parley::StyleProperty::FontSize(size));
    let mut layout = builder.build(text);
    layout.break_all_lines(None);
    let measured = (layout.width(), layout.height());
    if let Ok(mut map) = cache.lock() {
        map.insert(key, measured);
    }
    Some(measured)
}
/// Rasterise `text` at `px`, or nothing when the font stack cannot be built.
pub fn rasterize(text: &str, px: f32, color: [f32; 4]) -> Option<Raster> {
    let key = (text.to_string(), px.max(1.0).to_bits());
    let mut guard = engine().lock().ok()?;
    let engine = guard.as_mut()?;
    if !engine.cache.contains_key(&key) {
        let raster = engine.rasterize(text, px, color);
        engine.cache.insert(key.clone(), raster);
    }
    let cached = engine.cache.get(&key)?;
    Some(Raster {
        width: cached.width,
        height: cached.height,
        pixels: cached.pixels.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::rasterize;

    #[test]
    fn a_string_becomes_pixels_with_ink() {
        let Some(raster) = rasterize("Hello", 24.0, [0.0, 0.0, 0.0, 1.0]) else {
            // No system font on this machine: the renderer falls back to no text.
            return;
        };
        assert!(raster.width > 10, "width {}", raster.width);
        assert!(raster.height > 10, "height {}", raster.height);
        let inked = raster
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] > 0)
            .count();
        assert!(inked > 100, "only {inked} covered pixels");
    }

    #[test]
    fn the_same_string_is_rasterised_once() {
        let Some(first) = rasterize("cached", 16.0, [0.0, 0.0, 0.0, 1.0]) else {
            return;
        };
        let second = rasterize("cached", 16.0, [0.0, 0.0, 0.0, 1.0]).expect("cached");
        assert_eq!(first.width, second.width);
        assert_eq!(first.pixels, second.pixels);
    }
}
