//! The theme palette, from Microsoft's own `generic.xaml`.
//!
//! WinUI 3's default colours are not in any XBF this project can read (see
//! `docs/openxaml-scope.md`), but they are in Microsoft's MIT-licensed source: the theme
//! dictionaries in `dxaml/xcp/dxaml/themes/generic.xaml` define them, and `palette.txt` is
//! that file flattened -- `StaticResource` aliases followed -- into three columns:
//! theme, key, #AARRGGBB. `scripts/palette.py` regenerates it.
//!
//! The preview draws the dark theme, because the renderer's background is dark.

use std::collections::HashMap;
use std::sync::OnceLock;

const SOURCE: &str = include_str!("palette.txt");

/// The colour a theme key names in the dark theme, straight RGBA in 0..1.
pub fn color(key: &str) -> Option<[f32; 4]> {
    static TABLE: OnceLock<HashMap<&'static str, [f32; 4]>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let mut table = HashMap::new();
        for line in SOURCE.lines() {
            let mut fields = line.split('\t');
            let (Some(theme), Some(key), Some(value)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            if theme != "dark" {
                continue;
            }
            if let Some(color) = parse(value) {
                table.insert(key, color);
            }
        }
        table
    });
    table.get(key).copied()
}

/// `#AARRGGBB` as straight RGBA in 0..1, the order the element model stores.
fn parse(value: &str) -> Option<[f32; 4]> {
    let hex = value.strip_prefix('#')?;
    let argb = u32::from_str_radix(hex, 16).ok()?;
    let channel = |shift: u32| ((argb >> shift) & 0xFF) as f32 / 255.0;
    Some([channel(16), channel(8), channel(0), channel(24)])
}

#[cfg(test)]
mod tests {
    use super::color;

    /// The values are Microsoft's, so the test checks the two the templates ask for first.
    #[test]
    fn the_dark_theme_has_the_button_colours() {
        let background = color("ButtonBackground").expect("ButtonBackground");
        let foreground = color("ButtonForeground").expect("ButtonForeground");
        assert!(
            background[3] > 0.0 && background[3] < 1.0,
            "semi-transparent: {background:?}"
        );
        assert_eq!(foreground, [1.0, 1.0, 1.0, 1.0]);
    }
}
