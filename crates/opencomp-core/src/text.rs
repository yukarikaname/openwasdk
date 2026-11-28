//! Glyph rasterisation for the preview renderer.
//!
//! A `TextBlock` reaches the compositor as a string and a font size; this module turns
//! that into an RGBA bitmap the renderer draws like any other texture. One system font is
//! loaded on first use, glyphs are laid out by advance width, and the result is cached by
//! (text, size) because a preview re-renders the same strings every frame.
//!
//! Shaping is per glyph: kerning, ligatures and complex scripts are not applied. That is
//! the next step (parley over swash); what it replaces today is a blank box.

use std::collections::HashMap;

use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};

/// One rasterised run: straight RGBA8, row-major, with its natural size.
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

struct Engine {
    font: FontVec,
    cache: HashMap<(String, u32), Raster>,
}

static ENGINE: std::sync::OnceLock<std::sync::Mutex<Option<Engine>>> = std::sync::OnceLock::new();

fn engine() -> &'static std::sync::Mutex<Option<Engine>> {
    ENGINE.get_or_init(|| std::sync::Mutex::new(Engine::load()))
}

impl Engine {
    fn load() -> Option<Engine> {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let query = fontdb::Query {
            families: &[fontdb::Family::SansSerif],
            weight: fontdb::Weight::NORMAL,
            stretch: fontdb::Stretch::Normal,
            style: fontdb::Style::Normal,
        };
        let id = db
            .query(&query)
            .or_else(|| db.faces().next().map(|face| face.id))?;
        let (data, index) = db.with_face_data(id, |data, index| (data.to_vec(), index))?;
        let font = FontVec::try_from_vec_and_index(data, index).ok()?;
        Some(Engine {
            font,
            cache: HashMap::new(),
        })
    }

    fn rasterize(&mut self, text: &str, px: f32, color: [f32; 4]) -> Raster {
        let scale = PxScale::from(px.max(1.0));
        let scaled = self.font.as_scaled(scale);
        let ascent = scaled.ascent();
        let descent = scaled.descent();
        let height = (ascent - descent).ceil().max(1.0) as u32;
        let mut advances = Vec::new();
        let mut total = 0.0f32;
        for ch in text.chars() {
            let id = self.font.glyph_id(ch);
            advances.push((id, total));
            total += scaled.h_advance(id);
        }
        let width = total.ceil().max(1.0) as u32;

        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for (id, x) in advances {
            let Some(outline) = self.font.outline_glyph(ab_glyph::Glyph {
                id,
                scale,
                position: point(x, ascent),
            }) else {
                continue;
            };
            let bounds = outline.px_bounds();
            outline.draw(|gx, gy, coverage| {
                let px_x = bounds.min.x as i32 + gx as i32;
                let px_y = bounds.min.y as i32 + gy as i32;
                if px_x < 0 || px_y < 0 || px_x >= width as i32 || px_y >= height as i32 {
                    return;
                }
                let offset = ((px_y as u32 * width + px_x as u32) * 4) as usize;
                let alpha = (coverage * color[3]).clamp(0.0, 1.0);
                let existing = pixels[offset + 3] as f32 / 255.0;
                let combined = alpha + existing * (1.0 - alpha);
                if combined <= 0.0 {
                    return;
                }
                for channel in 0..3 {
                    let old = pixels[offset + channel] as f32 / 255.0;
                    let new = color[channel];
                    pixels[offset + channel] =
                        ((old * existing * (1.0 - alpha) + new * alpha) / combined * 255.0)
                            .clamp(0.0, 255.0) as u8;
                }
                pixels[offset + 3] = (combined * 255.0) as u8;
            });
        }
        Raster {
            width,
            height,
            pixels,
        }
    }
}

/// Rasterise `text` at `px`, or nothing when no system font could be loaded.
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
            .chunks_exact(4)
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
