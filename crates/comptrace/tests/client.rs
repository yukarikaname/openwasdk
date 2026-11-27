//! End-to-end check that the client view of a small namespace list is real Rust: generate it
//! into a crate under `target/` and let `cargo check` decide.
//!
//! The test skips when the Windows target or the local metadata is missing, because neither is
//! part of a plain checkout.

use std::path::{Path, PathBuf};
use std::process::Command;

const TARGET: &str = "x86_64-pc-windows-msvc";

// The list the client needs, plus the types windows-bindgen's closure leaves behind: the event
// argument types of TextBlock, the method types of IApplicationOverrides, and the Windows types
// named inside otherwise selected signatures.
const NAMESPACES: &str = concat!(
    "Microsoft.UI.Xaml.Controls.TextBlock,",
    "Microsoft.UI.Xaml.Application,",
    "Windows.Foundation,",
    "Windows.UI.Color,",
    "Windows.Foundation.Numerics,",
    "!Windows.Foundation.Diagnostics,",
    "Microsoft.UI.Xaml.Input,",
    "Microsoft.UI.Xaml.Media,",
    "Microsoft.UI.Xaml.Documents,",
    "Microsoft.UI.Xaml.RoutedEventArgs,",
    "Microsoft.UI.Xaml.BringIntoViewRequestedEventArgs,",
    "Microsoft.UI.Xaml.DataContextChangedEventArgs,",
    "Microsoft.UI.Xaml.DragStartingEventArgs,",
    "Microsoft.UI.Xaml.DropCompletedEventArgs,",
    "Microsoft.UI.Xaml.EffectiveViewportChangedEventArgs,",
    "Microsoft.UI.Xaml.Controls.IsTextTrimmedChangedEventArgs,",
    "Microsoft.UI.Xaml.ResourceManagerRequestedEventArgs,",
    "Microsoft.UI.Xaml.LaunchActivatedEventArgs,",
    "Microsoft.UI.Xaml.FocusVisualKind,",
    "Microsoft.UI.Xaml.ApplicationHighContrastAdjustment,",
    "Microsoft.UI.Xaml.UnhandledExceptionEventHandler,",
    "Microsoft.UI.Input,",
    "Windows.Storage.Streams.IBuffer,",
    "Windows.ApplicationModel.DataTransfer.DataPackageOperation",
);

const METADATA: &str = "/tmp/winmd";

fn metadata_files() -> Vec<PathBuf> {
    [
        "Microsoft.UI.Xaml.winmd",
        "Microsoft.UI.winmd",
        "Microsoft.Foundation.winmd",
        "Microsoft.Graphics.winmd",
        "Windows.winmd",
        "metadata/Microsoft.Windows.ApplicationModel.Resources.winmd",
    ]
    .iter()
    .map(|name| Path::new(METADATA).join(name))
    .collect()
}

fn target_installed() -> bool {
    let output = Command::new("rustc")
        .args(["--print", "target-libdir", "--target", TARGET])
        .output();
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let dir = String::from_utf8_lossy(&output.stdout);
    Path::new(dir.trim()).is_dir()
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cargo_command() -> Command {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    Command::new(cargo)
}

#[test]
fn the_client_view_of_a_small_filter_compiles() {
    if !Path::new(METADATA).is_dir() {
        eprintln!("skipping: {METADATA} does not exist");
        return;
    }
    let files = metadata_files();
    if let Some(missing) = files.iter().find(|path| !path.is_file()) {
        eprintln!("skipping: {} is missing", missing.display());
        return;
    }
    if !target_installed() {
        eprintln!("skipping: the {TARGET} target is not installed");
        return;
    }

    let crate_dir = workspace_root().join("target/comptrace-client");
    let source_dir = crate_dir.join("src");
    std::fs::create_dir_all(&source_dir).unwrap();
    let source = source_dir.join("lib.rs");

    std::fs::write(
        crate_dir.join("Cargo.toml"),
        concat!(
            "[package]\n",
            "name = \"comptrace-client\"\n",
            "version = \"0.0.0\"\n",
            "edition = \"2024\"\n\n",
            "[workspace]\n\n",
            "[dependencies]\n",
            "windows-core = \"0.100\"\n",
            "windows-collections = \"0.100\"\n",
            "windows-future = \"0.100\"\n",
            "windows-numerics = \"0.100\"\n",
            "windows-reference = \"0.100\"\n",
            "windows-time = \"0.100\"\n",
        ),
    )
    .unwrap();

    let mut command = Command::new(env!("CARGO_BIN_EXE_comptrace"));
    command.arg("generate");
    for file in &files {
        command.arg(file);
    }
    command
        .args(["--namespace", NAMESPACES, "--client", "--out"])
        .arg(&source);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "generate failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let text = std::fs::read_to_string(&source).unwrap();
    assert!(text.contains("pub struct TextBlock(windows_core::IUnknown);"));
    assert!(text.contains("pub fn SetText("));
    assert!(text.contains("pub trait IApplicationOverrides_Impl"));
    assert!(text.contains("fn OnLaunched("));

    let check = cargo_command()
        .args(["check", "--quiet", "--target", TARGET])
        .current_dir(&crate_dir)
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "cargo check failed:\n{}{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );

    // The same generation run against the file it just wrote must be a no-op, and one edited
    // line must be reported as stale with a non-zero status.
    let mut command = Command::new(env!("CARGO_BIN_EXE_comptrace"));
    command.arg("generate");
    for file in &files {
        command.arg(file);
    }
    command
        .args(["--namespace", NAMESPACES, "--client", "--check", "--out"])
        .arg(&source);
    let clean = command.output().unwrap();
    assert!(
        clean.status.success(),
        "--check rejected its own output:\n{}",
        String::from_utf8_lossy(&clean.stderr)
    );

    let mut stale = text;
    stale.push_str("\n// edited\n");
    std::fs::write(&source, stale).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_comptrace"));
    command.arg("generate");
    for file in &files {
        command.arg(file);
    }
    command
        .args(["--namespace", NAMESPACES, "--client", "--check", "--out"])
        .arg(&source);
    let dirty = command.output().unwrap();
    assert!(!dirty.status.success());
    let stderr = String::from_utf8_lossy(&dirty.stderr);
    assert!(stderr.contains("is stale"), "unexpected stderr: {stderr}");
}
