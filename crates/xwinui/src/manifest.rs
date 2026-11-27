//! The application manifest that lets a client activate a class from our DLL by name.

use std::fs;
use std::path::Path;

/// The DLL this project builds as the composition replacement.
pub const DEFAULT_DLL: &str = "opencomp.dll";

/// The classes a replacement runtime must serve before a WinUI 3 client starts.
pub const DEFAULT_CLASSES: &[&str] = &[
    "Microsoft.UI.Composition.Compositor",
    "Microsoft.UI.Content.ContentIsland",
    "Microsoft.UI.Content.DesktopChildSiteBridge",
];

/// Write the manifest, falling back to [`DEFAULT_CLASSES`] when no class is given.
pub fn write(out: &Path, dll: &str, classes: &[String]) -> Result<(), String> {
    let selected: Vec<&str> = if classes.is_empty() {
        DEFAULT_CLASSES.to_vec()
    } else {
        classes.iter().map(String::as_str).collect()
    };
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(out, render(dll, &selected))
        .map_err(|error| format!("write {}: {error}", out.display()))
}

/// Render the assembly document. The entries are the fragment Windows and Wine
/// look for under `<assembly>`; an XML declaration keeps the file loadable on
/// its own.
pub fn render(dll: &str, classes: &[&str]) -> String {
    let mut text = String::new();
    text.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    text.push_str(
        "<assembly manifestVersion=\"1.0\" xmlns=\"urn:schemas-microsoft-com:asm.v1\">\n",
    );
    text.push_str(&format!("  <file name=\"{dll}\">\n"));
    for class in classes {
        text.push_str(&format!(
            "    <activatableClass name=\"{class}\" threadingModel=\"both\" />\n"
        ));
    }
    text.push_str("  </file>\n");
    text.push_str("</assembly>\n");
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_matches_the_text_format() {
        let text = render(DEFAULT_DLL, DEFAULT_CLASSES);
        let expected = r#"<?xml version="1.0" encoding="utf-8"?>
<assembly manifestVersion="1.0" xmlns="urn:schemas-microsoft-com:asm.v1">
  <file name="opencomp.dll">
    <activatableClass name="Microsoft.UI.Composition.Compositor" threadingModel="both" />
    <activatableClass name="Microsoft.UI.Content.ContentIsland" threadingModel="both" />
    <activatableClass name="Microsoft.UI.Content.DesktopChildSiteBridge" threadingModel="both" />
  </file>
</assembly>
"#;
        assert_eq!(text, expected);
    }

    #[test]
    fn a_given_class_replaces_the_defaults() {
        let text = render("other.dll", &["Example.Class"]);
        assert!(text.contains("<file name=\"other.dll\">"));
        assert!(
            text.contains("<activatableClass name=\"Example.Class\" threadingModel=\"both\" />")
        );
        assert!(!text.contains("Microsoft.UI.Composition.Compositor"));
    }
}
