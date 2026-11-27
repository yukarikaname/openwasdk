//! The framework package, whose `AppxManifest.xml` is the runtime-class table.
//!
//! The SDK's own MSIX lists every class the runtime registers and the DLL that serves it
//! -- 1033 classes in 29 DLLs for the version this crate pins -- and nothing else in the
//! NuGet set carries that table. `activation` reads it to write the manifest Wine and
//! Windows both need for an unpackaged process.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::packages::{self, Package};

/// The package that carries one MSIX per architecture.
pub const PACKAGE: Package = Package {
    id: "microsoft.windowsappsdk.runtime",
    version: "2.7.0",
};

/// The file inside the package, for every architecture.
const FRAMEWORK: &str = "/Microsoft.WindowsAppRuntime.2.msix";

/// Unpack the framework MSIX for `arch` into `out` and return where it went.
pub fn run(out: &Path, cache: &Path, arch: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(out).map_err(|error| format!("create {}: {error}", out.display()))?;
    let nupkg = packages::ensure(&PACKAGE, cache)?;
    let file = File::open(&nupkg).map_err(|error| format!("open {}: {error}", nupkg.display()))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("read {}: {error}", nupkg.display()))?;
    let directory = format!("tools/MSIX/win10-{arch}/");
    let target = out.join(FRAMEWORK.trim_start_matches('/'));
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("read {} entry {index}: {error}", PACKAGE.id))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry
            .name()
            .map_err(|error| format!("read {} entry {index}: {error}", PACKAGE.id))?
            .to_string();
        if !name.ends_with(FRAMEWORK) || !name.starts_with(&directory) {
            continue;
        }
        let mut writer = File::create(&target)
            .map_err(|error| format!("create {}: {error}", target.display()))?;
        io::copy(&mut entry, &mut writer)
            .and_then(|_| writer.flush())
            .map_err(|error| format!("write {}: {error}", target.display()))?;
        return Ok(target);
    }
    Err(format!(
        "{} carries no win10-{arch} framework MSIX",
        PACKAGE.id
    ))
}
