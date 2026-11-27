//! The NuGet packages a WinUI 3 build needs and the cache in front of them.

use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};

/// One package, pinned by version so a layout does not move under the build.
pub struct Package {
    pub id: &'static str,
    pub version: &'static str,
}

impl Package {
    /// The flat-container URL that serves the `.nupkg` directly, with no client or feed API.
    pub fn url(&self) -> String {
        format!(
            "https://api.nuget.org/v3-flatcontainer/{id}/{version}/{id}.{version}.nupkg",
            id = self.id,
            version = self.version
        )
    }

    /// The cache file for this package.
    pub fn cached(&self, cache: &Path) -> PathBuf {
        cache.join(format!("{}.{}.nupkg", self.id, self.version))
    }
}

/// The four packages that carry the WinUI 3 metadata, headers and import libraries.
///
/// The Windows SDK projection is the large one; `c/UnionMetadata` in it holds
/// `Windows.winmd`, which the App SDK packages do not ship.
pub const PACKAGES: &[Package] = &[
    Package {
        id: "microsoft.windowsappsdk.interactiveexperiences",
        version: "2.7.0",
    },
    Package {
        id: "microsoft.windowsappsdk.winui",
        version: "2.7.0",
    },
    Package {
        id: "microsoft.windowsappsdk.foundation",
        version: "2.7.0",
    },
    Package {
        id: "microsoft.windows.sdk.cpp",
        version: "10.0.26100.4948",
    },
];

/// `~/.cache/xwinui`, or `%USERPROFILE%\.cache\xwinui` when `HOME` is absent.
pub fn default_cache() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or("cannot find the home directory; pass --cache")?;
    Ok(PathBuf::from(home).join(".cache").join("xwinui"))
}

/// Return the cached package, downloading it only when the file is not there.
///
/// The download goes to a `.part` file first, so an interrupted run leaves the
/// cache untouched instead of a truncated zip that fails later.
pub fn ensure(package: &Package, cache: &Path) -> Result<PathBuf, String> {
    let stored = package.cached(cache);
    if stored.is_file() {
        return Ok(stored);
    }
    fs::create_dir_all(cache).map_err(|error| format!("create {}: {error}", cache.display()))?;
    let url = package.url();
    eprintln!(
        "xwinui: downloading {} {} ({url})",
        package.id, package.version
    );
    let response = ureq::get(&url)
        .call()
        .map_err(|error| format!("download {url}: {error}"))?;
    let partial = stored.with_extension("nupkg.part");
    let file =
        File::create(&partial).map_err(|error| format!("create {}: {error}", partial.display()))?;
    let mut writer = BufWriter::new(file);
    let mut reader = response.into_body().into_reader();
    let bytes =
        io::copy(&mut reader, &mut writer).map_err(|error| format!("download {url}: {error}"))?;
    drop(writer);
    if bytes < 4 {
        let _ = fs::remove_file(&partial);
        return Err(format!("download {url}: empty response"));
    }
    fs::rename(&partial, &stored)
        .map_err(|error| format!("rename {}: {error}", partial.display()))?;
    eprintln!("xwinui: cached {} ({bytes} bytes)", stored.display());
    Ok(stored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_is_the_flat_container_form() {
        let url = PACKAGES[0].url();
        assert_eq!(
            url,
            "https://api.nuget.org/v3-flatcontainer/microsoft.windowsappsdk.interactiveexperiences/2.7.0/microsoft.windowsappsdk.interactiveexperiences.2.7.0.nupkg"
        );
    }

    #[test]
    fn cached_name_keeps_id_and_version() {
        let path = PACKAGES[3].cached(Path::new("/cache"));
        assert_eq!(
            path,
            Path::new("/cache/microsoft.windows.sdk.cpp.10.0.26100.4948.nupkg")
        );
    }
}
