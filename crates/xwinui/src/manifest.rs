//! The application manifest that lets a client activate a class from our DLL by name.
//!
//! This file is the registration a packaged or unpackaged application ships. The
//! process's activation context maps each `activatableClass` name to [`DEFAULT_DLL`],
//! and combase then loads that DLL and calls its `DllGetActivationFactory` when the
//! application asks for one of them. An application embeds the document as its
//! `RT_MANIFEST` resource, or places it beside the executable as
//! `<executable>.manifest`; either way the DLL has to be findable by name, which in
//! practice means beside the executable, because the entries name the file and
//! nothing else.

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

/// Escape the five predefined XML entities in an attribute value or text.
pub(crate) fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Render the assembly document. The entries are the fragment Windows and Wine
/// look for under `<assembly>`; an XML declaration keeps the file loadable on
/// its own.
///
/// `activatableClass` lives in the WinRT namespace, `urn:schemas-microsoft-com:winrt.v1`,
/// because that is the element both the Windows SxS parser and Wine's look for
/// under `<file>`; in the default `asm.v1` namespace the document parses as an
/// assembly with no activatable classes. `assemblyIdentity` makes it a complete
/// assembly rather than a fragment.
///
/// Each element of `files` becomes one `<file>`; the caller decides the order
/// of the files and of the classes inside them.
pub fn render_files(files: &[(String, Vec<String>)]) -> String {
    let mut text = String::new();
    text.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    text.push_str(
        "<assembly manifestVersion=\"1.0\" xmlns=\"urn:schemas-microsoft-com:asm.v1\">\n",
    );
    text.push_str("  <assemblyIdentity version=\"1.0.0.0\" name=\"OpenWASDK.Client\" />\n");
    for (dll, classes) in files {
        text.push_str(&format!("  <file name=\"{}\">\n", escape(dll)));
        for class in classes {
            text.push_str(&format!(
                "    <activatableClass name=\"{}\" threadingModel=\"both\" xmlns=\"urn:schemas-microsoft-com:winrt.v1\" />\n",
                escape(class)
            ));
        }
        text.push_str("  </file>\n");
    }
    text.push_str("</assembly>\n");
    text
}

/// Render one `<file>` for the given DLL and classes.
pub fn render(dll: &str, classes: &[&str]) -> String {
    let classes: Vec<String> = classes.iter().map(|class| (*class).to_string()).collect();
    render_files(&[(dll.to_string(), classes)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_matches_the_text_format() {
        let text = render(DEFAULT_DLL, DEFAULT_CLASSES);
        let expected = r#"<?xml version="1.0" encoding="utf-8"?>
<assembly manifestVersion="1.0" xmlns="urn:schemas-microsoft-com:asm.v1">
  <assemblyIdentity version="1.0.0.0" name="OpenWASDK.Client" />
  <file name="opencomp.dll">
    <activatableClass name="Microsoft.UI.Composition.Compositor" threadingModel="both" xmlns="urn:schemas-microsoft-com:winrt.v1" />
    <activatableClass name="Microsoft.UI.Content.ContentIsland" threadingModel="both" xmlns="urn:schemas-microsoft-com:winrt.v1" />
    <activatableClass name="Microsoft.UI.Content.DesktopChildSiteBridge" threadingModel="both" xmlns="urn:schemas-microsoft-com:winrt.v1" />
  </file>
</assembly>
"#;
        assert_eq!(text, expected);
    }

    #[test]
    fn every_class_is_in_the_winrt_namespace() {
        let text = render(DEFAULT_DLL, DEFAULT_CLASSES);
        let entries = text.matches("<activatableClass ").count();
        let namespaced = text.matches("urn:schemas-microsoft-com:winrt.v1").count();
        assert_eq!(entries, DEFAULT_CLASSES.len());
        assert_eq!(namespaced, entries);
    }

    #[test]
    fn a_given_class_replaces_the_defaults() {
        let text = render("other.dll", &["Example.Class"]);
        assert!(text.contains("<file name=\"other.dll\">"));
        assert!(text.contains("<activatableClass name=\"Example.Class\""));
        assert!(text.contains("threadingModel=\"both\""));
        assert!(!text.contains("Microsoft.UI.Composition.Compositor"));
    }

    /// What `xwinui manifest` leaves a client to ship: one file in the client's
    /// directory, naming the DLL that answers for the classes.
    #[test]
    fn a_client_directory_gets_the_manifest_to_ship() {
        let dir = std::env::temp_dir().join(format!("xwinui-manifest-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let out = dir.join("opencomp-client.exe.manifest");
        write(&out, DEFAULT_DLL, &[]).expect("write");
        let text = fs::read_to_string(&out).expect("read back");
        assert_eq!(text, render(DEFAULT_DLL, DEFAULT_CLASSES));
        let _ = fs::remove_dir_all(&dir);
    }
}
