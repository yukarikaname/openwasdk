//! What a PE image imports and exports.
//!
//! The import table is how a dependency question is answered without a debugger:
//! which DLL is needed, and which of its names is actually used. Both halves are
//! read from the file, so no Windows host is involved.

use std::fmt::Write as _;
use std::path::Path;

use object::Object;

/// One DLL and the names the image takes from it.
pub struct Dependency {
    pub dll: String,
    pub names: Vec<String>,
}

/// A name the image publishes.
pub struct Export {
    pub name: String,
    pub address: u64,
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn imports(path: &Path) -> Result<Vec<Dependency>, String> {
    let bytes = read(path)?;
    let file =
        object::File::parse(&*bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut table: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for import in file.imports().map_err(|error| error.to_string())? {
        let dll = String::from_utf8_lossy(import.library()).to_string();
        let name = String::from_utf8_lossy(import.name()).to_string();
        if !name.is_empty() {
            table.entry(dll).or_default().push(name);
        }
    }
    let mut out: Vec<Dependency> = table
        .into_iter()
        .map(|(dll, mut names)| {
            names.sort();
            names.dedup();
            Dependency { dll, names }
        })
        .collect();
    out.sort_by_key(|dependency| dependency.dll.to_lowercase());
    Ok(out)
}

pub fn exports(path: &Path) -> Result<Vec<Export>, String> {
    let bytes = read(path)?;
    let file =
        object::File::parse(&*bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    let mut out = Vec::new();
    for export in file.exports().map_err(|error| error.to_string())? {
        let name = String::from_utf8_lossy(export.name()).to_string();
        if !name.is_empty() {
            out.push(Export {
                name,
                address: export.address(),
            });
        }
    }
    out.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(out)
}

/// The import table as a report, optionally only the DLLs whose name contains `filter`.
pub fn report_imports(dependencies: &[Dependency], filter: Option<&str>) -> String {
    let selected: Vec<&Dependency> = dependencies
        .iter()
        .filter(|dependency| {
            filter.is_none_or(|filter| {
                dependency
                    .dll
                    .to_lowercase()
                    .contains(&filter.to_lowercase())
            })
        })
        .collect();
    if selected.is_empty() {
        return "no imports\n".to_string();
    }
    let mut out = String::new();
    let names: usize = selected
        .iter()
        .map(|dependency| dependency.names.len())
        .sum();
    let _ = writeln!(out, "{} DLLs, {names} names", selected.len());
    for dependency in selected {
        let _ = writeln!(out, "{} ({})", dependency.dll, dependency.names.len());
        for name in &dependency.names {
            let _ = writeln!(out, "    {name}");
        }
    }
    out
}

/// The export table as a report.
pub fn report_exports(exports: &[Export]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{} exports", exports.len());
    for export in exports {
        let _ = writeln!(out, "{:08x}  {}", export.address, export.name);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dependencies() -> Vec<Dependency> {
        vec![
            Dependency {
                dll: "KERNEL32.dll".to_string(),
                names: vec!["CloseHandle".to_string(), "CreateFileW".to_string()],
            },
            Dependency {
                dll: "Microsoft.Internal.FrameworkUdk.dll".to_string(),
                names: vec!["Windowing_GetWindowIdFromWindow".to_string()],
            },
        ]
    }

    #[test]
    fn every_dll_is_listed_with_its_names() {
        let report = report_imports(&dependencies(), None);
        assert!(report.starts_with("2 DLLs, 3 names"));
        assert!(report.contains("KERNEL32.dll (2)"));
        assert!(report.contains("    CloseHandle"));
    }

    #[test]
    fn a_filter_selects_by_substring_ignoring_case() {
        let report = report_imports(&dependencies(), Some("frameworkudk"));
        assert!(report.starts_with("1 DLLs, 1 names"));
        assert!(report.contains("Windowing_GetWindowIdFromWindow"));
        assert!(!report.contains("KERNEL32"));
    }

    #[test]
    fn a_filter_that_matches_nothing_says_so() {
        assert_eq!(
            report_imports(&dependencies(), Some("dcomp")),
            "no imports\n"
        );
    }

    #[test]
    fn exports_are_printed_with_their_address() {
        let report = report_exports(&[Export {
            name: "DllGetActivationFactory".to_string(),
            address: 0x1234,
        }]);
        assert!(report.starts_with("1 exports"));
        assert!(report.contains("00001234  DllGetActivationFactory"));
    }
}
