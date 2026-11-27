//! Put the App SDK's runtime DLLs where a client can load them.
//!
//! `layout` places headers and import libraries for a compiler; this places the files
//! that exist at run time. They live under `runtimes-framework/win-x64/native` in the
//! four packages -- not `runtimes/`, which only carries the bootstrapper -- and the
//! `dcompi.dll` in that set is the DLL our compositor replaces.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::packages::{self, Package};

/// Where a client's DLLs are after a fetch.
pub struct Summary {
    pub dlls: usize,
    pub metadata: usize,
    pub out: PathBuf,
}

/// Fetch the four packages and unpack the runtime files into `out`.
pub fn run(out: &Path, cache: &Path, metadata: bool) -> Result<Summary, String> {
    fs::create_dir_all(out).map_err(|error| format!("create {}: {error}", out.display()))?;
    let mut dlls = 0;
    let mut metadata_files = 0;
    for package in packages::RUNTIME {
        let nupkg = packages::ensure(package, cache)?;
        let (unpacked_dlls, unpacked_metadata) = unpack(package, &nupkg, out, metadata)?;
        dlls += unpacked_dlls;
        metadata_files += unpacked_metadata;
    }
    Ok(Summary {
        dlls,
        metadata: metadata_files,
        out: out.to_path_buf(),
    })
}

/// Unpack one package and report what it contributed.
fn unpack(
    package: &Package,
    nupkg: &Path,
    out: &Path,
    metadata: bool,
) -> Result<(usize, usize), String> {
    let file = File::open(nupkg).map_err(|error| format!("open {}: {error}", nupkg.display()))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("read {}: {error}", nupkg.display()))?;
    let mut dlls = 0;
    let mut metadata_files = 0;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("read {} entry {index}: {error}", package.id))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry
            .name()
            .map_err(|error| format!("read {} entry {index}: {error}", package.id))?
            .to_string();
        let keep = if is_runtime_dll(&name) {
            dlls += 1;
            true
        } else if metadata && is_metadata(&name) {
            metadata_files += 1;
            true
        } else {
            false
        };
        if !keep {
            continue;
        }
        // Flat: the client loads these by name from beside its own executable, and the
        // localized resource folders below `native/` are not part of that.
        let Some(file_name) = name.rsplit('/').next() else {
            continue;
        };
        let destination = out.join(file_name);
        if destination.exists() {
            continue;
        }
        let mut target = File::create(&destination)
            .map_err(|error| format!("create {}: {error}", destination.display()))?;
        io::copy(&mut entry, &mut target)
            .map_err(|error| format!("write {}: {error}", destination.display()))?;
    }
    Ok((dlls, metadata_files))
}

/// A DLL a client loads at run time.
///
/// `runtimes-framework` holds the framework itself; `runtimes` holds the bootstrapper,
/// which an unpackaged client needs and a self-contained one does not.
fn is_runtime_dll(path: &str) -> bool {
    let Some(rest) = path
        .strip_prefix("runtimes-framework/win-x64/native/")
        .or_else(|| path.strip_prefix("runtimes/win-x64/native/"))
    else {
        return false;
    };
    // One directory level at most: `native/foo.dll` yes, `native/de-DE/foo.dll.mui` no.
    !rest.contains('/') && rest.ends_with(".dll")
}

/// The WinRT metadata a generator reads, which the same packages carry.
fn is_metadata(path: &str) -> bool {
    path.starts_with("metadata/") && path.ends_with(".winmd") && !path.contains("..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_flat_native_dlls_are_runtime_files() {
        assert!(is_runtime_dll(
            "runtimes-framework/win-x64/native/Microsoft.ui.xaml.dll"
        ));
        assert!(is_runtime_dll(
            "runtimes/win-x64/native/Microsoft.WindowsAppRuntime.Bootstrap.dll"
        ));
        assert!(!is_runtime_dll(
            "runtimes-framework/win-x64/native/de-DE/Microsoft.ui.xaml.dll.mui"
        ));
        assert!(!is_runtime_dll("runtimes-framework/win-arm64/native/a.dll"));
        assert!(!is_runtime_dll("lib/net6.0-windows/Microsoft.WinUI.dll"));
        assert!(!is_runtime_dll(
            "runtimes-framework/win-x64/native/readme.txt"
        ));
    }

    #[test]
    fn metadata_is_the_winmd_under_metadata() {
        assert!(is_metadata("metadata/Microsoft.UI.Xaml.winmd"));
        assert!(is_metadata("metadata/10.0.18362.0/Microsoft.UI.winmd"));
        assert!(!is_metadata("lib/Microsoft.UI.Xaml.dll"));
        assert!(!is_metadata("metadata/readme.txt"));
    }
}
