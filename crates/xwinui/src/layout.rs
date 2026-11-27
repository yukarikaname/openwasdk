//! Turn the NuGet packages into one directory tree that clang-cl can consume.
//!
//! Only files a compiler reads are copied: metadata, headers and import
//! libraries. Package-relative paths are kept below `<id>/<version>/`, which
//! also keeps two packages from writing the same path.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::Path;

use serde_json::{Value, json};
use zip::ZipArchive;

use crate::check::REQUIRED_WINMD;
use crate::packages::{self, Package};

const LAYOUT_JSON: &str = "layout.json";
const LAYOUT_README: &str = "README.txt";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Winmd,
    Header,
    Library,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Winmd => "winmd",
            Kind::Header => "header",
            Kind::Library => "library",
        }
    }
}

struct Entry {
    package: &'static str,
    package_path: String,
    path: String,
    kind: Kind,
    size: u64,
}

/// What a completed layout contains, for the caller to print.
pub struct Summary {
    pub packages: usize,
    pub files: usize,
    pub bytes: u64,
}

/// Download what is missing, extract everything a compiler reads, then write
/// the two index files that describe the result.
pub fn run(out: &Path, cache: &Path) -> Result<Summary, String> {
    fs::create_dir_all(out).map_err(|error| format!("create {}: {error}", out.display()))?;
    let mut entries = Vec::new();
    let mut packages_json = Vec::new();
    for package in packages::PACKAGES {
        let nupkg = packages::ensure(package, cache)?;
        entries.extend(extract(package, &nupkg, out)?);
        packages_json.push(json!({
            "id": package.id,
            "version": package.version,
            "url": package.url(),
        }));
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));

    let files: Vec<Value> = entries
        .iter()
        .map(|entry| {
            json!({
                "package": entry.package,
                "package_path": entry.package_path,
                "path": entry.path,
                "kind": entry.kind.as_str(),
                "size": entry.size,
            })
        })
        .collect();
    let document = json!({
        "generator": "xwinui",
        "packages": packages_json,
        "files": files,
    });
    let text = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("encode {LAYOUT_JSON}: {error}"))?;
    fs::write(out.join(LAYOUT_JSON), text + "\n")
        .map_err(|error| format!("write {}: {error}", out.join(LAYOUT_JSON).display()))?;
    write_readme(out, &entries)?;

    let bytes = entries.iter().map(|entry| entry.size).sum();
    Ok(Summary {
        packages: packages::PACKAGES.len(),
        files: entries.len(),
        bytes,
    })
}

/// Read one `.nupkg` and copy the matching members, keeping their paths.
fn extract(package: &Package, nupkg: &Path, out: &Path) -> Result<Vec<Entry>, String> {
    let file = File::open(nupkg).map_err(|error| format!("open {}: {error}", nupkg.display()))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("open {}: {error}", nupkg.display()))?;
    let root = out.join(package.id).join(package.version);
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut member = archive
            .by_index(index)
            .map_err(|error| format!("read {}: {error}", nupkg.display()))?;
        if member.is_dir() {
            continue;
        }
        let Some(relative) = member.enclosed_name() else {
            continue;
        };
        let package_path = relative.to_string_lossy().replace('\\', "/");
        let Some(kind) = classify(&package_path) else {
            continue;
        };
        let destination = root.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("create {}: {error}", parent.display()))?;
        }
        let target = File::create(&destination)
            .map_err(|error| format!("create {}: {error}", destination.display()))?;
        let mut writer = BufWriter::new(target);
        let size = io::copy(&mut member, &mut writer)
            .map_err(|error| format!("extract {package_path}: {error}"))?;
        entries.push(Entry {
            package: package.id,
            package_path: package_path.clone(),
            path: format!("{}/{}/{}", package.id, package.version, package_path),
            kind,
            size,
        });
    }
    Ok(entries)
}

/// Decide whether a package member is something a compiler reads. The Windows
/// SDK nests its tree under `c/` (`c/Include`, `c/Lib`, `c/UnionMetadata`), so
/// look one segment deeper there; the App SDK packages are flat.
fn classify(path: &str) -> Option<Kind> {
    let extension = path.rsplit_once('.').map(|(_, extension)| extension)?;
    let mut segments = path.split('/');
    let first = segments.next()?;
    let root = if first.eq_ignore_ascii_case("c") {
        segments.next().unwrap_or("")
    } else {
        first
    };
    let union = path
        .split('/')
        .any(|segment| segment.eq_ignore_ascii_case("UnionMetadata"));
    if extension.eq_ignore_ascii_case("winmd")
        && (root.eq_ignore_ascii_case("metadata") || root.eq_ignore_ascii_case("lib") || union)
    {
        return Some(Kind::Winmd);
    }
    if root.eq_ignore_ascii_case("include")
        && ["h", "hpp", "hh", "hxx", "inl"]
            .iter()
            .any(|wanted| extension.eq_ignore_ascii_case(wanted))
    {
        return Some(Kind::Header);
    }
    if root.eq_ignore_ascii_case("lib") && extension.eq_ignore_ascii_case("lib") {
        return Some(Kind::Library);
    }
    None
}

fn write_readme(out: &Path, entries: &[Entry]) -> Result<(), String> {
    let mut text = String::from(
        "OpenWASDK xwinui SDK layout\n\
         ===========================\n\n\
         Generated by `xwinui layout`. Every package keeps its NuGet-relative\n\
         paths below <package id>/<version>/, so two packages cannot collide.\n\
         layout.json lists every extracted file with its package and kind.\n\n\
         Packages:\n",
    );
    for package in packages::PACKAGES {
        let _ = writeln!(text, "  {} {}", package.id, package.version);
    }

    text.push_str("\nMetadata (.winmd):\n");
    for name in REQUIRED_WINMD {
        match find_named(entries, name) {
            Some(entry) => {
                let _ = writeln!(text, "  {name:<24} {}", entry.path);
            }
            None => {
                let _ = writeln!(text, "  {name:<24} MISSING");
            }
        }
    }

    let includes = directories(entries, |entry| entry.kind == Kind::Header);
    let libraries = directories(entries, |entry| {
        entry.kind == Kind::Library && entry.path.split('/').any(|part| part == "x64")
    });
    text.push_str(
        "\nA clang-cl command for an x64 target. Add more include directories if\n\
         the compiler asks for a header that is not on this list; layout.json has\n\
         all of them.\n\n",
    );
    let mut command = vec![
        "clang-cl".to_string(),
        "/std:c++20".to_string(),
        "/EHsc".to_string(),
        "/MD".to_string(),
    ];
    for directory in &includes {
        command.push(format!("/I {directory}"));
    }
    command.push("your-app.cpp".to_string());
    if !libraries.is_empty() {
        command.push("/link".to_string());
        for directory in &libraries {
            command.push(format!("/LIBPATH:{directory}"));
        }
    }
    text.push_str("  ");
    text.push_str(&command.join(" ^\n      "));
    text.push('\n');

    text.push_str(
        "\nMetadata is also what cppwinrt and comptrace read:\n\n  \
         cppwinrt -in <the .winmd paths above> -out generated\n  \
         cargo run -p comptrace -- summary <a .winmd path>\n\n\
         The Windows SDK package in this layout ships headers and metadata but\n\
         no import libraries; add the Windows SDK .lib files your link needs.\n\n\
         Run `xwinui check --layout .` to see which required files are present.\n",
    );
    fs::write(out.join(LAYOUT_README), text)
        .map_err(|error| format!("write {}: {error}", out.join(LAYOUT_README).display()))
}

fn find_named<'a>(entries: &'a [Entry], name: &str) -> Option<&'a Entry> {
    entries.iter().rfind(|entry| {
        entry
            .path
            .rsplit_once('/')
            .map(|(_, file)| file)
            .is_some_and(|file| file == name)
    })
}

/// Unique parent directories of the entries matching `keep`, in path order.
fn directories(entries: &[Entry], keep: impl Fn(&Entry) -> bool) -> Vec<String> {
    let mut found = BTreeSet::new();
    for entry in entries.iter().filter(|entry| keep(entry)) {
        if let Some((directory, _)) = entry.path.rsplit_once('/') {
            found.insert(directory.to_string());
        }
    }
    found.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_keeps_only_compiler_inputs() {
        assert_eq!(
            classify("metadata/10.0.18362.0/Microsoft.UI.winmd"),
            Some(Kind::Winmd)
        );
        assert_eq!(
            classify("c/UnionMetadata/10.0.26100.0/Windows.winmd"),
            Some(Kind::Winmd)
        );
        assert_eq!(classify("include/winrt/base.h"), Some(Kind::Header));
        assert_eq!(
            classify("c/Include/10.0.26100.0/cppwinrt/winrt/base.h"),
            Some(Kind::Header)
        );
        assert_eq!(classify("include/readme.idl"), None);
        assert_eq!(classify("lib/native/x64/MRM.lib"), Some(Kind::Library));
        assert_eq!(
            classify("c/Lib/10.0.26100.0/um/x64/kernel32.lib"),
            Some(Kind::Library)
        );
        assert_eq!(classify("runtimes/win-x64/native/foo.dll"), None);
    }

    #[test]
    fn layout_downloads_and_lists_the_metadata() {
        let root = std::env::temp_dir().join(format!("xwinui-layout-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let out = root.join("sdk");
        let cache = packages::default_cache().unwrap_or_else(|_| root.join("cache"));
        match run(&out, &cache) {
            Ok(summary) => {
                let json = fs::read_to_string(out.join(LAYOUT_JSON)).unwrap();
                for name in REQUIRED_WINMD {
                    assert!(json.contains(name), "layout.json does not name {name}");
                }
                assert!(out.join(LAYOUT_README).is_file());
                assert!(summary.files > 0);
                let report = crate::check::report(&out).unwrap();
                assert_eq!(report.missing, 0, "{}", report.text);
            }
            Err(message) if message.starts_with("download ") => {
                println!("skipping layout test: network unavailable ({message})");
            }
            Err(message) => panic!("layout failed: {message}"),
        }
        let _ = fs::remove_dir_all(&root);
    }
}
