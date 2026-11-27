//! Strings inside an image, which is how a DLL says what it knows about classes.
//!
//! A WinRT server carries the class names it answers for as UTF-16 strings, and a
//! client carries the ones it activates. Grepping for a namespace prefix is therefore
//! the shortest way to answer "who serves this class" without a debugger.

use std::path::Path;

/// A string found in the image and how many times it occurs.
#[derive(Debug, PartialEq)]
pub struct Found {
    pub text: String,
    pub count: usize,
}

fn printable(byte: u8) -> bool {
    (0x20..0x7f).contains(&byte)
}

/// Accumulate one occurrence, merging repeats.
fn add(found: &mut Vec<Found>, text: String) {
    match found.iter_mut().find(|entry| entry.text == text) {
        Some(entry) => entry.count += 1,
        None => found.push(Found { text, count: 1 }),
    }
}

fn collect_wide(data: &[u8], needle: &[u8]) -> Vec<Found> {
    let mut found = Vec::new();
    let mut at = 0;
    while at + needle.len() <= data.len() {
        let Some(offset) = data[at..]
            .windows(needle.len())
            .position(|window| window == needle)
        else {
            break;
        };
        at += offset;
        // A UTF-16 character is a printable ASCII byte followed by a zero byte.
        let unit = |index: usize| -> bool {
            index + 1 < data.len() && printable(data[index]) && data[index + 1] == 0
        };
        let mut start = at;
        while start >= 2 && unit(start - 2) {
            start -= 2;
        }
        let mut end = at;
        while end + 1 < data.len() && unit(end) {
            end += 2;
        }
        if end > start {
            let units: Vec<u16> = data[start..end]
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect();
            add(&mut found, String::from_utf16_lossy(&units));
        }
        at = end.max(at + 2);
    }
    found
}

fn collect_narrow(data: &[u8], needle: &[u8]) -> Vec<Found> {
    let mut found = Vec::new();
    let mut at = 0;
    while at + needle.len() <= data.len() {
        let Some(offset) = data[at..]
            .windows(needle.len())
            .position(|window| window == needle)
        else {
            break;
        };
        at += offset;
        let mut start = at;
        while start >= 1 && printable(data[start - 1]) {
            start -= 1;
        }
        let mut end = at;
        while end < data.len() && printable(data[end]) {
            end += 1;
        }
        if end > start {
            add(
                &mut found,
                String::from_utf8_lossy(&data[start..end]).to_string(),
            );
        }
        at = end.max(at + 1);
    }
    found
}

/// Every string that starts with `needle`, wide (`UTF-16LE`) or narrow.
pub fn find(data: &[u8], needle: &str, wide: bool) -> Vec<Found> {
    if needle.is_empty() {
        return Vec::new();
    }
    let mut found = if wide {
        let mut bytes = Vec::with_capacity(needle.len() * 2);
        for unit in needle.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        collect_wide(data, &bytes)
    } else {
        collect_narrow(data, needle.as_bytes())
    };
    found.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then(left.text.cmp(&right.text))
    });
    found
}

pub fn image(path: &Path, needle: &str, wide: bool) -> Result<Vec<Found>, String> {
    let data = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(find(&data, needle, wide))
}

pub fn report(found: &[Found]) -> String {
    if found.is_empty() {
        return "nothing\n".to_string();
    }
    let mut rows: Vec<&Found> = found.iter().collect();
    rows.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then(left.text.cmp(&right.text))
    });
    let mut out = format!("{} strings\n", rows.len());
    for entry in rows {
        out.push_str(&format!("  x{:<4} {}\n", entry.count, entry.text));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(text: &str) -> Vec<u8> {
        let mut bytes = Vec::new();
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes.extend_from_slice(&[0, 0]);
        bytes
    }

    fn image_of(strings: &[&str]) -> Vec<u8> {
        let mut data = vec![0u8; 4];
        for text in strings {
            data.extend_from_slice(&wide(text));
            data.extend_from_slice(&[0, 0]);
        }
        data
    }

    #[test]
    fn a_wide_string_is_found_whole() {
        let data = image_of(&["Microsoft.UI.Content.ContentIsland", "other"]);
        let found = find(&data, "Microsoft.UI.Content.", true);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text, "Microsoft.UI.Content.ContentIsland");
        assert_eq!(found[0].count, 1);
    }

    #[test]
    fn a_repeated_string_is_counted_once() {
        let data = image_of(&[
            "Microsoft.UI.Composition.Compositor",
            "Microsoft.UI.Composition.Compositor",
        ]);
        let found = find(&data, "Microsoft.UI.Composition.", true);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].count, 2);
    }

    #[test]
    fn a_narrow_string_is_found_too() {
        let mut data = vec![0u8; 3];
        data.extend_from_slice(b"Microsoft.UI.Xaml.Controls.Button\0");
        let found = find(&data, "Microsoft.UI.Xaml.", false);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text, "Microsoft.UI.Xaml.Controls.Button");
    }

    #[test]
    fn a_prefix_that_matches_nothing_reports_nothing() {
        let data = image_of(&["Microsoft.UI.Xaml"]);
        assert!(find(&data, "Microsoft.UI.Content.", true).is_empty());
    }

    #[test]
    fn the_report_orders_by_count() {
        let found = vec![
            Found {
                text: "b".to_string(),
                count: 1,
            },
            Found {
                text: "a".to_string(),
                count: 3,
            },
        ];
        let report = report(&found);
        assert!(report.starts_with("2 strings"));
        assert!(report.find("x3 ").unwrap() < report.find("x1 ").unwrap());
    }
}
