//! Audit a layout, so a missing metadata file or header names itself before a compile.

use std::fs;
use std::path::{Path, PathBuf};

/// Metadata every WinUI 3 build needs: the App SDK surface, the Win32 projection
/// and `Microsoft.Graphics` (DirectX interop types are in the composition ABI).
pub const REQUIRED_WINMD: &[&str] = &[
    "Microsoft.UI.winmd",
    "Windows.winmd",
    "Microsoft.Graphics.winmd",
];

/// Headers for the bootstrap, the composition interop boundary and the XAML host.
pub const REQUIRED_HEADERS: &[&str] = &[
    "MddBootstrap.h",
    "Microsoft.UI.Interop.h",
    "Microsoft.UI.Composition.Interop.h",
    "microsoft.ui.xaml.window.h",
    "microsoft.ui.xaml.hosting.referencetracker.h",
];

/// What `check` found. `missing` is the number of required names not present, so
/// the caller can turn the report into a non-zero exit status.
pub struct Report {
    pub text: String,
    pub missing: usize,
}

/// Scan the layout and describe every required file as present or missing.
pub fn report(layout: &Path) -> Result<Report, String> {
    let files = walk(layout)?;
    let mut text = format!("layout {}\n", layout.display());
    let mut missing = 0;

    text.push_str("metadata:\n");
    let (block, absent) = section(&files, REQUIRED_WINMD);
    text.push_str(&block);
    missing += absent;

    text.push_str("headers:\n");
    let (block, absent) = section(&files, REQUIRED_HEADERS);
    text.push_str(&block);
    missing += absent;

    let total = REQUIRED_WINMD.len() + REQUIRED_HEADERS.len();
    text.push_str(&format!("{missing} of {total} required file(s) missing\n"));
    Ok(Report { text, missing })
}

/// Format one group of required names, aligned so a missing entry stands out.
fn section(files: &[PathBuf], names: &[&str]) -> (String, usize) {
    let width = names.iter().map(|name| name.len()).max().unwrap_or(0);
    let mut text = String::new();
    let mut missing = 0;
    for name in names {
        let found: Vec<&PathBuf> = files
            .iter()
            .filter(|path| path.file_name().and_then(|file| file.to_str()) == Some(name))
            .collect();
        match found.last() {
            None => {
                missing += 1;
                text.push_str(&format!("  {:<7}  {:<width$}\n", "MISSING", name));
            }
            Some(path) => {
                text.push_str(&format!(
                    "  {:<7}  {:<width$}  {}",
                    "present",
                    name,
                    path.display()
                ));
                if found.len() > 1 {
                    text.push_str(&format!(" (+{} more)", found.len() - 1));
                }
                text.push('\n');
            }
        }
    }
    (text, missing)
}

/// Every regular file below the layout, sorted so the report is stable.
fn walk(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    visit(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn visit(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|error| format!("read {}: {error}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("read {}: {error}", dir.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("stat {}: {error}", path.display()))?;
        if file_type.is_dir() {
            visit(&path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_winmd_is_named() {
        let root = std::env::temp_dir().join(format!("xwinui-check-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let metadata = root.join("package/metadata");
        fs::create_dir_all(&metadata).unwrap();
        fs::write(metadata.join("Windows.winmd"), b"").unwrap();
        fs::write(metadata.join("Microsoft.Graphics.winmd"), b"").unwrap();
        for header in REQUIRED_HEADERS {
            let path = root.join("package/include").join(header);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"").unwrap();
        }

        let report = report(&root).unwrap();
        assert!(report.text.contains("MISSING  Microsoft.UI.winmd"));
        let total = REQUIRED_WINMD.len() + REQUIRED_HEADERS.len();
        assert!(report.text.contains(&format!("1 of {total} required")));
        assert_eq!(report.missing, 1);

        fs::remove_dir_all(&root).unwrap();
    }
}
