//! The `generate` subcommand's argument plumbing: namespaces from a file, the client view, and
//! the staleness check that keeps a committed `bindings.rs` honest.
//!
//! windows-bindgen only writes to a file, so `--check` generates into a scratch file under the
//! system temporary directory, compares, and removes it.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Everything one `generate` run needs, already resolved from the command line.
pub struct Options<'a> {
    pub inputs: &'a [PathBuf],
    pub namespaces: &'a [String],
    pub implement: &'a [String],
    pub out: &'a Path,
    pub minimal: bool,
    pub client: bool,
}

/// Join `--namespace` and `--namespace-file`, keeping the command line first.
///
/// Blank lines and lines starting with `#` are ignored so the file can carry a comment.
pub fn namespaces(from_args: &[String], file: Option<&Path>) -> Result<Vec<String>, String> {
    let mut names = from_args.to_vec();
    if let Some(path) = file {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            names.push(line.to_string());
        }
    }
    let mut seen = HashSet::new();
    names.retain(|name| seen.insert(name.clone()));
    Ok(names)
}

/// Translate the options into the argv windows-bindgen expects.
///
/// An `--implement` with no names means every selected interface, which is what `--client`
/// wants: the server only implements, the client also needs the traits to implement itself.
pub fn args(options: &Options<'_>) -> Vec<String> {
    let mut args = Vec::new();
    for path in options.inputs {
        args.push("--in".to_string());
        args.push(path.to_string_lossy().into_owned());
    }
    args.push("--out".to_string());
    args.push(options.out.to_string_lossy().into_owned());
    args.push("--filter".to_string());
    args.extend(options.namespaces.iter().cloned());
    args.push("--implement".to_string());
    if !options.client {
        args.extend(options.implement.iter().cloned());
    }
    if options.minimal && !options.client {
        args.push("--minimal".to_string());
    }
    args
}

/// A unique path for the output of a run that must not touch the real file.
pub fn scratch() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    std::env::temp_dir().join(format!("comptrace-check-{}-{nanos}.rs", std::process::id()))
}

/// Compare a generated file with the committed one, or explain the first difference in one line.
pub fn compare(expected: &Path, generated: &Path) -> Result<(), String> {
    let generated_text = std::fs::read_to_string(generated)
        .map_err(|error| format!("{}: {error}", generated.display()))?;
    let expected_text = match std::fs::read_to_string(expected) {
        Ok(text) => text,
        Err(error) => {
            return Err(format!("{} is stale: {error}", expected.display()));
        }
    };
    if expected_text == generated_text {
        return Ok(());
    }
    Err(format!(
        "{} is stale: {}",
        expected.display(),
        difference(&expected_text, &generated_text)
    ))
}

/// `N line(s) added, M removed, first difference at line L`, counting common ends only.
fn difference(expected: &str, generated: &str) -> String {
    let old: Vec<&str> = expected.lines().collect();
    let new: Vec<&str> = generated.lines().collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let removed = old.len() - prefix - suffix;
    let added = new.len() - prefix - suffix;
    format!(
        "{added} line(s) added, {removed} removed, first difference at line {}",
        prefix + 1
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_namespace_file_adds_entries_and_skips_comments() {
        let path = scratch().with_extension("txt");
        std::fs::write(
            &path,
            "# client\nMicrosoft.UI.Xaml.Application\n\nWindows.Foundation\n",
        )
        .unwrap();
        let names = namespaces(
            &[
                "Windows.UI.Color".to_string(),
                "Windows.Foundation".to_string(),
            ],
            Some(&path),
        )
        .unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(
            names,
            [
                "Windows.UI.Color",
                "Windows.Foundation",
                "Microsoft.UI.Xaml.Application"
            ]
        );
    }

    #[test]
    fn a_client_run_drops_minimal_and_implements_everything() {
        let names = vec!["Microsoft.UI.Xaml.Application".to_string()];
        let implements = vec!["Windows.Foundation".to_string()];
        let options = Options {
            inputs: &[],
            namespaces: &names,
            implement: &implements,
            out: Path::new("bindings.rs"),
            minimal: true,
            client: true,
        };
        let args = args(&options);
        assert!(!args.iter().any(|arg| arg == "--minimal"));
        let implement = args.iter().position(|arg| arg == "--implement").unwrap();
        assert_eq!(implement + 1, args.len());
    }

    #[test]
    fn the_check_names_one_difference() {
        assert_eq!(
            difference("one\ntwo\nthree\n", "one\nTWO\nthree\n"),
            "1 line(s) added, 1 removed, first difference at line 2"
        );
        assert_eq!(
            difference("one\n", "one\ntwo\n"),
            "1 line(s) added, 0 removed, first difference at line 2"
        );
    }
}
