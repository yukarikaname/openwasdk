//! End-to-end check that `scaffold` emits the shape the committed `bindings.rs` expects:
//! generate the traits for one class's interfaces, then scaffold the class and compile both.
//!
//! The test skips when the Windows target or the local metadata is missing, like `client.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

const TARGET: &str = "x86_64-pc-windows-msvc";
const METADATA: &str = "/tmp/winmd";

// ContainerVisual and the interfaces it inherits from Visual and CompositionObject.
const FILTER: &str = concat!(
    "Microsoft.UI.Composition.IContainerVisual,",
    "Microsoft.UI.Composition.IAnimationObject,",
    "Microsoft.UI.Composition.ICompositionObject,",
    "Microsoft.UI.Composition.ICompositionObject2,",
    "Microsoft.UI.Composition.ICompositionObject3,",
    "Microsoft.UI.Composition.ICompositionObject4,",
    "Microsoft.UI.Composition.ICompositionObject5,",
    "Microsoft.UI.Composition.IVisual,",
    "Microsoft.UI.Composition.IVisual2,",
    "Microsoft.UI.Composition.IVisual3,",
    "Microsoft.UI.Composition.IVisual4,",
    "Windows.Foundation.IClosable",
);

fn metadata_files() -> Vec<PathBuf> {
    [
        "Microsoft.UI.winmd",
        "Microsoft.Graphics.winmd",
        "Microsoft.Foundation.winmd",
        "Windows.winmd",
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

fn run(command: &mut Command) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{} failed:\n{}",
        command.get_program().to_string_lossy(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_scaffold_compiles_against_the_generated_bindings() {
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

    let crate_dir = workspace_root().join("target/comptrace-scaffold");
    let source_dir = crate_dir.join("src");
    std::fs::create_dir_all(&source_dir).unwrap();
    std::fs::write(
        crate_dir.join("Cargo.toml"),
        concat!(
            "[package]\n",
            "name = \"comptrace-scaffold\"\n",
            "version = \"0.0.0\"\n",
            "edition = \"2024\"\n\n",
            "[workspace]\n\n",
            "[dependencies]\n",
            "windows-core = \"0.100\"\n",
            "windows-numerics = \"0.100\"\n",
        ),
    )
    .unwrap();

    let bindings = source_dir.join("bindings.rs");
    let mut generate = Command::new(env!("CARGO_BIN_EXE_comptrace"));
    generate.arg("generate");
    for file in &files {
        generate.arg(file);
    }
    generate
        .args(["--namespace", FILTER, "--minimal", "--out"])
        .arg(&bindings);
    run(&mut generate);

    let scaffold = source_dir.join("scaffold.rs");
    let mut command = Command::new(env!("CARGO_BIN_EXE_comptrace"));
    command.arg("scaffold");
    for file in &files {
        command.arg(file);
    }
    command
        .args([
            "--class",
            "Microsoft.UI.Composition.ContainerVisual",
            "--out",
        ])
        .arg(&scaffold);
    run(&mut command);

    std::fs::write(
        source_dir.join("lib.rs"),
        "pub mod bindings;\nmod scaffold;\n",
    )
    .unwrap();

    let text = std::fs::read_to_string(&scaffold).unwrap();
    assert!(text.contains("#[windows_core::implement("));
    assert!(text.contains("IContainerVisual_Impl for ContainerVisual_Impl"));
    assert!(text.contains("fn Children(&self)"));
    assert!(!text.contains("unsafe {"));
    assert!(!text.contains("#[allow"));

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let check = Command::new(cargo)
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
}
