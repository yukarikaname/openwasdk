//! Build a WinUI 3 .NET project on a host that cannot execute a Windows PE.
//!
//! The WinUI 3 targets run the XAML compiler as a separate process:
//! `Microsoft.UI.Xaml.Markup.Compiler.interop.targets` (in the
//! `microsoft.windowsappsdk.winui` package) names it with
//! `XamlCompilerExePath`, defaulting to `tools/net472/XamlCompiler.exe`, and
//! calls it from an `Exec`. That file is a .NET Framework PE, so `dotnet build`
//! on macOS or Linux dies at `MarkupCompilePass1`. The fix is to overwrite the
//! property with an executable shell script that hands the same arguments to
//! Wine.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Failure;

/// The package whose `tools/net472` holds the compiler.
const COMPILER_PACKAGE: &str = "microsoft.windowsappsdk.winui";

/// The property the targets read, and its default value inside the package.
pub const COMPILER_PROPERTY: &str = "XamlCompilerExePath";
const COMPILER_RELATIVE: &str = "tools/net472/XamlCompiler.exe";

/// WinUI 3 native assets only ship per architecture, so the build needs a RID.
pub const DEFAULT_RID: &str = "win-x64";

/// The wrapper file name below `<cache>/bin`.
const WRAPPER: &str = "xamlcompiler-wine";

pub struct Request<'a> {
    pub project: &'a Path,
    pub configuration: &'a str,
    pub wine: Option<&'a Path>,
    pub cache: &'a Path,
}

/// Look for Wine, restore if the compiler package is not extracted yet, write
/// the wrapper, then build with [`COMPILER_PROPERTY`] pointed at it.
pub fn run(request: &Request) -> Result<(), Failure> {
    let wine = find_wine(request.wine).ok_or_else(missing_wine)?;
    let compiler = ensure_compiler(request)?;
    let wrapper = write_wrapper(request.cache, &wine, &compiler)?;
    println!("wine     {}", wine.display());
    println!("compiler {}", compiler.display());
    println!("wrapper  {}", wrapper.display());

    let status = Command::new("dotnet")
        .arg("build")
        .arg(request.project)
        .arg("--configuration")
        .arg(request.configuration)
        .arg("--nologo")
        .arg("-p:EnableWindowsTargeting=true")
        .arg(format!("-p:RuntimeIdentifier={DEFAULT_RID}"))
        .arg(format!("-p:{COMPILER_PROPERTY}={}", wrapper.display()))
        .status()
        .map_err(|error| Failure::step(format!("run dotnet build: {error}"), 1))?;
    if !status.success() {
        return Err(Failure::step(format!("dotnet build failed ({status})"), 1));
    }
    println!(
        "built {} ({DEFAULT_RID}, {})",
        request.project.display(),
        request.configuration
    );
    Ok(())
}

/// The step that fails without Wine, named rather than reported as a build error.
fn missing_wine() -> Failure {
    Failure::step(
        "cannot run the XAML compiler: wine was not found\n  \
         step: MarkupCompilePass1 executes tools/net472/XamlCompiler.exe,\n       \
         a Windows PE (Microsoft.UI.Xaml.Markup.Compiler.interop.targets,\n       \
         property XamlCompilerExePath)\n  \
         fix:  install Wine, or pass --wine <path>",
        2,
    )
}

/// `--wine` wins; otherwise the first executable named `wine` on `PATH`.
fn find_wine(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return is_executable(path).then(|| path.to_path_buf());
    }
    let path = std::env::var_os("PATH")?;
    for directory in std::env::split_paths(&path) {
        for name in ["wine", "wine64", "wine-stable"] {
            let candidate = directory.join(name);
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

/// Find the restored compiler, running `dotnet restore` once when the package
/// is not in the global packages folder yet. The wrapper needs the real path.
fn ensure_compiler(request: &Request) -> Result<PathBuf, Failure> {
    let root = global_packages()?;
    let wanted = restored_version(request.project);
    if let Some(path) = compiler_in(&root, wanted.as_deref()) {
        return Ok(path);
    }
    eprintln!("xwinui: {COMPILER_PACKAGE} is not restored; running dotnet restore once");
    let status = Command::new("dotnet")
        .arg("restore")
        .arg(request.project)
        .arg("--nologo")
        .arg("-p:EnableWindowsTargeting=true")
        .arg(format!("-p:RuntimeIdentifier={DEFAULT_RID}"))
        .status()
        .map_err(|error| Failure::step(format!("run dotnet restore: {error}"), 1))?;
    if !status.success() {
        return Err(Failure::step(
            format!("dotnet restore failed ({status})"),
            1,
        ));
    }
    let wanted = restored_version(request.project);
    compiler_in(&root, wanted.as_deref()).ok_or_else(|| {
        Failure::step(
            format!(
                "cannot find {COMPILER_PACKAGE}/{COMPILER_RELATIVE} under {}",
                root.display()
            ),
            1,
        )
    })
}

/// `NUGET_PACKAGES`, else `~/.nuget/packages`.
fn global_packages() -> Result<PathBuf, Failure> {
    if let Some(path) = std::env::var_os("NUGET_PACKAGES") {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| Failure::step("cannot find the home directory for NuGet", 1))?;
    Ok(PathBuf::from(home).join(".nuget").join("packages"))
}

/// The version the project last restored, from `obj/project.assets.json`, so a
/// stale second copy in the package cache cannot be picked instead.
fn restored_version(project: &Path) -> Option<String> {
    let root = if project.is_dir() {
        project
    } else {
        project.parent()?
    };
    let text = fs::read_to_string(root.join("obj").join("project.assets.json")).ok()?;
    let document: serde_json::Value = serde_json::from_str(&text).ok()?;
    let prefix = format!("{COMPILER_PACKAGE}/");
    document
        .get("libraries")?
        .as_object()?
        .keys()
        .find_map(|key| key.strip_prefix(&prefix).map(str::to_string))
}

/// The compiler of `wanted` when present, else the highest restored version.
fn compiler_in(root: &Path, wanted: Option<&str>) -> Option<PathBuf> {
    let package = root.join(COMPILER_PACKAGE);
    if let Some(version) = wanted {
        let path = package.join(version).join(COMPILER_RELATIVE);
        if path.is_file() {
            return Some(path);
        }
    }
    let mut best: Option<([u64; 4], PathBuf)> = None;
    for entry in fs::read_dir(&package).ok()? {
        let Ok(entry) = entry else { continue };
        let path = entry.path().join(COMPILER_RELATIVE);
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let key = version_key(&name);
        if best.as_ref().is_none_or(|(best, _)| key > *best) {
            best = Some((key, path));
        }
    }
    best.map(|(_, path)| path)
}

/// `2.10.0` must sort above `2.7.0`, so compare numbers, not text.
fn version_key(version: &str) -> [u64; 4] {
    let mut key = [0; 4];
    for (slot, part) in key.iter_mut().zip(version.split(['.', '-', '+'])) {
        *slot = part.parse().unwrap_or(0);
    }
    key
}

/// Write the script below the cache and make it executable.
fn write_wrapper(cache: &Path, wine: &Path, compiler: &Path) -> Result<PathBuf, Failure> {
    let bin = cache.join("bin");
    fs::create_dir_all(&bin)
        .map_err(|error| Failure::step(format!("create {}: {error}", bin.display()), 1))?;
    let wrapper = bin.join(WRAPPER);
    let partial = wrapper.with_extension("part");
    let script = format!(
        "#!/bin/sh\n\
         # Generated by `xwinui build`; do not edit.\n\
         # The WinUI 3 targets execute net472/XamlCompiler.exe, a Windows PE.\n\
         # This host cannot execute it, so hand the arguments to Wine.\n\
         exec {} {} \"$@\"\n",
        shell_quote(wine),
        shell_quote(compiler),
    );
    fs::write(&partial, script)
        .map_err(|error| Failure::step(format!("write {}: {error}", partial.display()), 1))?;
    make_executable(&partial)?;
    fs::rename(&partial, &wrapper)
        .map_err(|error| Failure::step(format!("rename {}: {error}", partial.display()), 1))?;
    Ok(wrapper)
}

/// Single quotes, so spaces, `$` and backslashes stay literal to the shell.
fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', r"'\''"))
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), Failure> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)
        .map_err(|error| Failure::step(format!("stat {}: {error}", path.display()), 1))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|error| Failure::step(format!("chmod {}: {error}", path.display()), 1))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), Failure> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("xwinui-build-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(version_key("2.10.0") > version_key("2.7.0"));
        assert_eq!(version_key("2.7.0"), version_key("2.7.0.0"));
        assert_eq!(version_key("1.8.0-preview.2")[0], 1);
    }

    #[test]
    fn quoting_keeps_a_space_and_a_dollar_sign_literal() {
        assert_eq!(shell_quote(Path::new("/a b/$x")), "'/a b/$x'");
        assert_eq!(shell_quote(Path::new("/it's")), r"'/it'\''s'");
    }

    /// A shell script standing in for Wine, and one for the compiler, prove the
    /// wrapper is executable and forwards every argument unchanged.
    #[cfg(unix)]
    #[test]
    fn wrapper_forwards_arguments_through_a_script_wine() {
        let root = scratch("wrapper");
        let wine = root.join("fake wine");
        fs::write(&wine, "#!/bin/sh\nexec \"$@\"\n").unwrap();
        make_executable(&wine).unwrap();
        let compiler = root.join("fake compiler");
        fs::write(
            &compiler,
            "#!/bin/sh\nfor arg in \"$@\"; do printf '%s\\n' \"$arg\"; done\n",
        )
        .unwrap();
        make_executable(&compiler).unwrap();

        let wrapper = write_wrapper(&root.join("cache"), &wine, &compiler).unwrap();
        assert!(is_executable(&wrapper));
        let output = Command::new(&wrapper)
            .args(["input file.json", "$HOME", "it's", "out.json"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert_eq!(text, "input file.json\n$HOME\nit's\nout.json\n");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_missing_wine_exits_two_and_names_the_step() {
        let failure = run(&Request {
            project: Path::new("/nonexistent/project"),
            configuration: "Debug",
            wine: Some(Path::new("/nonexistent/wine")),
            cache: Path::new("/nonexistent/cache"),
        })
        .unwrap_err();
        assert_eq!(failure.code, 2);
        assert!(failure.message.contains("wine was not found"));
        assert!(failure.message.contains("XamlCompilerExePath"));
    }

    #[test]
    fn the_newest_restored_compiler_is_chosen() {
        let root = scratch("compiler");
        for version in ["2.7.0", "2.10.0"] {
            let path = root
                .join(COMPILER_PACKAGE)
                .join(version)
                .join(COMPILER_RELATIVE);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, b"").unwrap();
        }
        let found = compiler_in(&root, None).unwrap();
        assert!(found.to_string_lossy().contains("2.10.0"));

        let wanted = compiler_in(&root, Some("2.7.0")).unwrap();
        assert!(wanted.to_string_lossy().contains("2.7.0"));
        fs::remove_dir_all(&root).unwrap();
    }
}
