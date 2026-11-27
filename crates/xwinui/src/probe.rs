//! List what a finished build produced, so the next step has a checklist.
//!
//! After `xwinui build` the run step needs three things next to each other: the
//! application `.exe`, the managed and native `.dll` files it loads, and the
//! `.xbf` XAML the XAML engine reads. `probe` names each one with its size so a
//! missing file is visible before Wine is started.

use std::fs;
use std::path::{Path, PathBuf};

use crate::Failure;

/// Extensions the run step cares about, in the order they are grouped.
const KINDS: &[&str] = &["exe", "dll", "xbf"];

struct Found {
    path: PathBuf,
    kind: &'static str,
    size: u64,
}

/// Find the build output, walk it, and render the report.
pub fn report(project: &Path, configuration: &str) -> Result<String, Failure> {
    let root = project_root(project)?;
    let bin = root.join("bin");
    let output = output_dir(&bin, configuration).ok_or_else(|| {
        Failure::step(
            format!(
                "no build output under {}; run `xwinui build --project {}` first",
                bin.display(),
                root.display()
            ),
            1,
        )
    })?;
    let mut files = Vec::new();
    visit(&output, &mut files)?;
    if files.is_empty() {
        return Err(Failure::step(
            format!(
                "{} holds no .exe, .dll or .xbf file; the build may not have finished",
                output.display()
            ),
            1,
        ));
    }
    Ok(render(&root, &output, &files))
}

/// A directory argument is used as is; a project file names its directory.
fn project_root(project: &Path) -> Result<PathBuf, Failure> {
    if project.is_dir() {
        return Ok(project.to_path_buf());
    }
    project.parent().map(Path::to_path_buf).ok_or_else(|| {
        Failure::step(
            format!(
                "{} is neither a directory nor a project file",
                project.display()
            ),
            1,
        )
    })
}

/// `bin/<configuration>`; the TFM and RID are extra segments below it, and the
/// walk finds them. Printing a guessed `bin/<configuration>/<rid>` would miss
/// the target framework folder a .NET SDK project inserts.
fn output_dir(bin: &Path, configuration: &str) -> Option<PathBuf> {
    let plain = bin.join(configuration);
    plain.is_dir().then_some(plain)
}

/// Collect the matching files below `dir`, sorted by path.
fn visit(dir: &Path, found: &mut Vec<Found>) -> Result<(), Failure> {
    let entries = fs::read_dir(dir)
        .map_err(|error| Failure::step(format!("read {}: {error}", dir.display()), 1))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| Failure::step(format!("read {}: {error}", dir.display()), 1))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| Failure::step(format!("stat {}: {error}", path.display()), 1))?;
        if file_type.is_dir() {
            visit(&path, found)?;
            continue;
        }
        let Some(kind) = kind_of(&path) else {
            continue;
        };
        let size = entry
            .metadata()
            .map_err(|error| Failure::step(format!("stat {}: {error}", path.display()), 1))?
            .len();
        found.push(Found { path, kind, size });
    }
    found.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(())
}

/// The lowercase extension when it is one `probe` reports.
fn kind_of(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    KINDS.iter().copied().find(|kind| extension == *kind)
}

fn render(root: &Path, output: &Path, files: &[Found]) -> String {
    let mut text = format!(
        "project {}\noutput  {}\n\n",
        root.display(),
        output.display()
    );
    let mut total = 0;
    for file in files {
        // `/` on every platform: the report is read by people and compared in CI, and a
        // Windows checkout would otherwise list `themes\App.xbf` for the same file.
        // `/` on every platform: the report is read by people and compared in CI, and a
        // Windows checkout would otherwise list `themes\App.xbf` for the same file.
        let relative = file
            .path
            .strip_prefix(output)
            .unwrap_or(&file.path)
            .to_string_lossy()
            .replace('\\', "/");
        text.push_str(&format!(
            "  {:<4} {:>10}  {:>10}  {relative}\n",
            file.kind,
            file.size,
            human_size(file.size),
        ));
        total += file.size;
    }
    let count = files.len();
    text.push_str(&format!("\n{count} file(s), {total} byte(s)\n"));

    let application = files
        .iter()
        .find(|file| file.kind == "exe")
        .map(|file| file.path.display().to_string())
        .unwrap_or_else(|| "<application>.exe".to_string());
    text.push_str("\nchecklist for the run step:\n");
    text.push_str("  1. copy opencomp.dll, openudk.dll and the activation manifest\n");
    text.push_str("     next to the .exe\n");
    text.push_str("  2. run it under Wine:\n");
    text.push_str(&format!("     wine {application}\n"));
    text
}

/// Bytes below 1 KiB, then one decimal place, so the size reads at a glance.
fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_switch_unit_at_1024() {
        assert_eq!(human_size(344), "344 B");
        assert_eq!(human_size(2048), "2.0 KiB");
        assert_eq!(human_size(3 * 1024 * 1024), "3.0 MiB");
    }

    #[test]
    fn the_report_lists_only_the_three_kinds_with_sizes() {
        let root = std::env::temp_dir().join(format!("xwinui-probe-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let output = root.join("bin/Debug/win-x64");
        fs::create_dir_all(output.join("themes")).unwrap();
        fs::write(output.join("App.exe"), vec![0u8; 1024]).unwrap();
        fs::write(output.join("opencomp.dll"), vec![0u8; 2048]).unwrap();
        fs::write(output.join("themes/App.xbf"), vec![0u8; 344]).unwrap();
        fs::write(output.join("App.pdb"), b"debug symbols").unwrap();

        let text = report(&root, "Debug").unwrap();
        assert!(text.contains("1.0 KiB"), "{text}");
        assert!(text.contains("App.exe"));
        assert!(text.contains("opencomp.dll"));
        assert!(text.contains("themes/App.xbf"), "{text}");
        assert!(!text.contains("App.pdb"), "{text}");
        assert!(text.contains("3 file(s)"));
        assert!(text.contains("wine "));

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn an_unbuilt_project_is_an_error() {
        let root = std::env::temp_dir().join(format!("xwinui-probe-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let failure = report(&root, "Debug").unwrap_err();
        assert!(failure.message.contains("xwinui build"));
        fs::remove_dir_all(&root).unwrap();
    }
}
