//! Read the runtime-class table the Windows App SDK ships in its MSIX manifest
//! and turn it into the Win32 activation manifest Windows and Wine consume.
//!
//! The AppxManifest records each class as
//! `<ActivatableClass ActivatableClassId="…" />` inside an
//! `<Extension Category="windows.activatableClass.inProcessServer">`, with a
//! sibling `<Path>` naming the DLL that serves it. The scan is hand written
//! because the crate has no XML dependency; `appx` describes what it does not
//! handle (namespaces other than the default one, entities beyond the five XML
//! predefined ones). A truncated or mismatched document is an error, never a
//! partial table.
//!
//! `windows.activatableClass.proxyStub` extensions are recognised and skipped:
//! they register interfaces, not activatable classes. Other categories are
//! ignored.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::appx::{self, Node};
use crate::manifest;

/// Extension category for a DLL-hosted runtime class.
pub const IN_PROCESS: &str = "windows.activatableClass.inProcessServer";
/// Extension category for a server-hosted runtime class.
pub const OUT_OF_PROCESS: &str = "windows.activatableClass.outOfProcessServer";
/// Extension category for a proxy/stub DLL; it registers no classes.
pub const PROXY_STUB: &str = "windows.activatableClass.proxyStub";

const MANIFEST: &str = "AppxManifest.xml";

/// The category an entry was found under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    InProcessServer,
    OutOfProcessServer,
    ProxyStub,
}

impl Category {
    fn parse(label: &str) -> Option<Self> {
        match label {
            IN_PROCESS => Some(Self::InProcessServer),
            OUT_OF_PROCESS => Some(Self::OutOfProcessServer),
            PROXY_STUB => Some(Self::ProxyStub),
            _ => None,
        }
    }
}

/// One runtime class, the DLL the manifest says serves it, and its category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub class: String,
    pub dll: String,
    pub category: Category,
}

/// Parse an AppxManifest into its activatable classes.
pub fn parse(xml: &str) -> Result<Vec<Entry>, String> {
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(xml);
    let nodes = appx::scan(xml)?;
    appx::validate(&nodes)?;
    let closes = appx::matching(&nodes);
    let mut entries = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        let Node::Tag(tag) = node else {
            continue;
        };
        if tag.closing || tag.name != "Extension" {
            continue;
        }
        let Some(label) = tag.attribute("Category") else {
            continue;
        };
        let Some(category) = Category::parse(label) else {
            continue;
        };
        if category == Category::ProxyStub {
            continue;
        }
        let end = closes[index].ok_or_else(|| format!("extension {label} is not closed"))?;
        parse_extension(&nodes, index + 1, end, category, &mut entries)?;
    }
    Ok(entries)
}

/// Render the entries as one Win32 assembly, one `<file>` per DLL.
///
/// A replacement rule is a class name or class-name prefix plus the DLL that
/// should serve matching classes. An exact class wins over every prefix. If
/// several prefixes match, the longest wins, then the first in the list. A
/// class with no matching rule keeps the DLL named by the AppxManifest.
pub fn render(entries: &[Entry], replacements: &[(&str, &str)]) -> String {
    let mut files: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in entries {
        let dll = served_by(&entry.class, &entry.dll, replacements);
        files
            .entry(dll.to_string())
            .or_default()
            .push(entry.class.clone());
    }
    let files: Vec<(String, Vec<String>)> = files
        .into_iter()
        .map(|(dll, mut classes)| {
            classes.sort_unstable();
            classes.dedup();
            (dll, classes)
        })
        .collect();
    manifest::render_files(&files)
}

/// Pick the DLL for a class; see [`render`] for the precedence rules.
fn served_by<'a>(class: &str, dll: &'a str, replacements: &[(&'a str, &'a str)]) -> &'a str {
    for (pattern, replacement) in replacements {
        if *pattern == class {
            return replacement;
        }
    }
    let mut best: Option<(&str, &str)> = None;
    for (pattern, replacement) in replacements {
        if pattern.is_empty() || !class.starts_with(pattern) {
            continue;
        }
        if best.is_none_or(|(current, _)| pattern.len() > current.len()) {
            best = Some((pattern, replacement));
        }
    }
    best.map_or(dll, |(_, replacement)| replacement)
}

/// Read the AppxManifest from an SDK directory, an `.msix`, or a manifest file.
pub fn read(source: &Path) -> Result<String, String> {
    if source.is_dir() {
        let manifest = find(source, |path| {
            path.file_name().and_then(|name| name.to_str()) == Some(MANIFEST)
        })?;
        if let Some(path) = manifest {
            return fs::read_to_string(&path)
                .map_err(|error| format!("read {}: {error}", path.display()));
        }
        let msix = find(source, |path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("msix"))
        })?;
        return match msix {
            Some(path) => read_msix(&path),
            None => Err(format!("no {MANIFEST} or .msix below {}", source.display())),
        };
    }
    let is_manifest = source.file_name().and_then(|name| name.to_str()) == Some(MANIFEST);
    let is_xml = source
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("xml"));
    if is_manifest || is_xml {
        return fs::read_to_string(source)
            .map_err(|error| format!("read {}: {error}", source.display()));
    }
    read_msix(source)
}

/// Write the rendered manifest, creating the parent directory.
pub fn write(out: &Path, text: &str) -> Result<(), String> {
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(out, text).map_err(|error| format!("write {}: {error}", out.display()))
}

fn parse_extension(
    nodes: &[Node],
    start: usize,
    end: usize,
    category: Category,
    out: &mut Vec<Entry>,
) -> Result<(), String> {
    let mut dll: Option<String> = None;
    let mut classes: Vec<String> = Vec::new();
    let mut index = start;
    while index < end {
        let Node::Tag(tag) = &nodes[index] else {
            index += 1;
            continue;
        };
        if !tag.closing && tag.name == "Path" {
            if tag.self_closing {
                return Err("an activation extension has an empty <Path>".to_string());
            }
            let (value, close) = element_text(nodes, index, end)?;
            if value.is_empty() {
                return Err("an activation extension has an empty <Path>".to_string());
            }
            if dll.replace(value).is_some() {
                return Err("an activation extension has more than one <Path>".to_string());
            }
            index = close + 1;
            continue;
        }
        if !tag.closing && tag.name == "ActivatableClass" {
            let class = tag
                .attribute("ActivatableClassId")
                .ok_or("<ActivatableClass> has no ActivatableClassId")?;
            if class.is_empty() {
                return Err("<ActivatableClass> has an empty ActivatableClassId".to_string());
            }
            classes.push(class.to_string());
            index += 1;
            continue;
        }
        if !tag.closing && tag.name == "ActivatableClassId" {
            let (class, close) = element_text(nodes, index, end)?;
            if class.is_empty() {
                return Err("<ActivatableClassId> is empty".to_string());
            }
            classes.push(class);
            index = close + 1;
            continue;
        }
        index += 1;
    }
    let dll = dll.ok_or("an activation extension has no <Path>")?;
    if classes.is_empty() {
        return Err(format!("{dll} has no ActivatableClass entries"));
    }
    for class in classes {
        out.push(Entry {
            class,
            dll: dll.clone(),
            category,
        });
    }
    Ok(())
}

/// The text of an element that contains text only, and the index of its close.
fn element_text(nodes: &[Node], open: usize, limit: usize) -> Result<(String, usize), String> {
    let Node::Tag(start) = &nodes[open] else {
        return Err("element text requested at a text node".to_string());
    };
    let mut text = String::new();
    let mut index = open + 1;
    while index < limit {
        match &nodes[index] {
            Node::Text(value) => text.push_str(value),
            Node::Tag(tag) => {
                if tag.closing && tag.name == start.name {
                    return Ok((text.trim().to_string(), index));
                }
                return Err(format!(
                    "<{}> contains markup, which is not supported",
                    start.name
                ));
            }
        }
        index += 1;
    }
    Err(format!("<{}> is not closed", start.name))
}

fn find(root: &Path, wanted: impl Fn(&Path) -> bool) -> Result<Option<PathBuf>, String> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let mut children: Vec<PathBuf> = fs::read_dir(&dir)
            .map_err(|error| format!("read {}: {error}", dir.display()))?
            .map(|entry| {
                entry
                    .map(|entry| entry.path())
                    .map_err(|error| format!("read {}: {error}", dir.display()))
            })
            .collect::<Result<_, _>>()?;
        children.sort();
        for child in children {
            let metadata = fs::symlink_metadata(&child)
                .map_err(|error| format!("read {}: {error}", child.display()))?;
            if metadata.file_type().is_dir() {
                pending.push(child);
            } else if metadata.file_type().is_file() && wanted(&child) {
                return Ok(Some(child));
            }
        }
    }
    Ok(None)
}

fn read_msix(path: &Path) -> Result<String, String> {
    let file = File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| format!("read {} as a zip: {error}", path.display()))?;
    let mut member = archive
        .by_name(MANIFEST)
        .map_err(|error| format!("{} has no {MANIFEST}: {error}", path.display()))?;
    let mut text = String::new();
    member
        .read_to_string(&mut text)
        .map_err(|error| format!("read {MANIFEST} from {}: {error}", path.display()))?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10">
  <Extensions>
    <Extension Category="windows.activatableClass.inProcessServer">
      <InProcessServer>
        <Path>Microsoft.UI.Xaml.dll</Path>
        <ActivatableClass ActivatableClassId="Microsoft.UI.Xaml.Application" ThreadingModel="both" />
        <ActivatableClass ActivatableClassId="Microsoft.UI.Xaml.Window" ThreadingModel="both" />
      </InProcessServer>
    </Extension>
    <Extension Category="windows.activatableClass.inProcessServer">
      <InProcessServer>
        <Path>Microsoft.UI.Composition.dll</Path>
        <ActivatableClass ActivatableClassId="Microsoft.UI.Composition.Compositor" ThreadingModel="both" />
        <ActivatableClass ActivatableClassId="Microsoft.UI.Composition.Visual" ThreadingModel="both" />
      </InProcessServer>
    </Extension>
    <Extension Category="windows.activatableClass.proxyStub">
      <ProxyStub ClassId="71201C0F-21E1-4B80-945F-7B0A10F90883">
        <Path>Stub.dll</Path>
        <Interface Name="IThing" InterfaceId="25604D55-9B17-426F-9D67-2B11B3A65598" />
      </ProxyStub>
    </Extension>
  </Extensions>
</Package>
"#;

    fn entry(class: &str, dll: &str) -> Entry {
        Entry {
            class: class.to_string(),
            dll: dll.to_string(),
            category: Category::InProcessServer,
        }
    }

    #[test]
    fn scanner_reads_classes_over_two_dlls_and_ignores_proxy_stub() {
        let entries = parse(FIXTURE).expect("parse");
        assert_eq!(
            entries,
            vec![
                entry("Microsoft.UI.Xaml.Application", "Microsoft.UI.Xaml.dll"),
                entry("Microsoft.UI.Xaml.Window", "Microsoft.UI.Xaml.dll"),
                entry(
                    "Microsoft.UI.Composition.Compositor",
                    "Microsoft.UI.Composition.dll"
                ),
                entry(
                    "Microsoft.UI.Composition.Visual",
                    "Microsoft.UI.Composition.dll"
                ),
            ]
        );
    }

    #[test]
    fn a_replacement_owns_a_prefix_but_an_exact_class_wins() {
        let entries = vec![
            entry("Microsoft.UI.Composition.Compositor", "dcompi.dll"),
            entry("Microsoft.UI.Composition.Visual", "dcompi.dll"),
            entry("Microsoft.UI.Composition.ContainerVisual", "dcompi.dll"),
            entry("Microsoft.UI.Xaml.Application", "Microsoft.UI.Xaml.dll"),
        ];
        let text = render(
            &entries,
            &[
                ("Microsoft.UI.Composition.", "opencomp.dll"),
                ("Microsoft.UI.Composition.Compositor", "exact.dll"),
            ],
        );
        assert!(text.contains(
            "<file name=\"exact.dll\">\n    <activatableClass name=\"Microsoft.UI.Composition.Compositor\""
        ));
        assert!(text.contains(
            "<file name=\"Microsoft.UI.Xaml.dll\">\n    <activatableClass name=\"Microsoft.UI.Xaml.Application\""
        ));
        let opencomp = text
            .split("<file name=\"opencomp.dll\">")
            .nth(1)
            .expect("opencomp file");
        assert!(!opencomp.contains("Microsoft.UI.Composition.Compositor\""));
        assert!(
            opencomp.find("Microsoft.UI.Composition.ContainerVisual\"")
                < opencomp.find("Microsoft.UI.Composition.Visual\"")
        );
    }

    #[test]
    fn a_truncated_document_is_an_error() {
        let truncated = r#"<Package><Extensions>
            <Extension Category="windows.activatableClass.inProcessServer">
            <InProcessServer><Path>Microsoft.UI.Xaml.dll</Path>
            <ActivatableClass ActivatableClassId="Microsoft.UI.Xaml.Window" />"#;
        let error = parse(truncated).expect_err("truncated document");
        assert!(error.contains("not closed"), "{error}");
    }

    #[test]
    fn a_document_without_a_path_is_an_error() {
        let text = r#"<Package><Extension Category="windows.activatableClass.inProcessServer">
            <ActivatableClass ActivatableClassId="Example.Class" />
            </Extension></Package>"#;
        let error = parse(text).expect_err("missing path");
        assert!(error.contains("<Path>"), "{error}");
    }

    #[test]
    fn the_element_form_of_a_class_name_is_accepted() {
        let text = r#"<Package><Extension Category="windows.activatableClass.inProcessServer">
            <InProcessServer><Path>Example.dll</Path>
            <ActivatableClassId>Example.Class</ActivatableClassId>
            </InProcessServer></Extension></Package>"#;
        let entries = parse(text).expect("parse");
        assert_eq!(entries, vec![entry("Example.Class", "Example.dll")]);
    }

    #[test]
    fn an_out_of_process_extension_is_kept() {
        let text = r#"<Package><Extension Category="windows.activatableClass.outOfProcessServer">
            <OutOfProcessServer><Path>Server.exe</Path>
            <ActivatableClass ActivatableClassId="Example.Class" />
            </OutOfProcessServer></Extension></Package>"#;
        let entries = parse(text).expect("parse");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].category, Category::OutOfProcessServer);
        assert_eq!(entries[0].dll, "Server.exe");
    }
}
