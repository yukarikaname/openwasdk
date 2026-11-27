//! `layout-probe` asks the official `Microsoft.ui.xaml.dll` one layout question:
//! a vertical `StackPanel`, `Spacing = 4`, three 10 px high `Border` children.
//!
//! `XAML_DLL` names the DLL to load. `--out <file>` writes the one JSON line to a
//! file; without it the line goes to stdout. Every step reports its HRESULT to
//! stderr before the next call, so a fault still leaves a readable trail.

#[cfg(not(windows))]
fn main() {
    eprintln!(
        "layout-probe: this probe needs Windows or Wine; Microsoft.ui.xaml.dll is Windows only"
    );
    std::process::exit(1);
}

#[cfg(windows)]
fn main() {
    let out = match options() {
        Ok(out) => out,
        Err(message) => {
            eprintln!("layout-probe: {message}");
            std::process::exit(2);
        }
    };
    std::process::exit(windows::run(out));
}

#[cfg(windows)]
fn options() -> Result<Option<std::path::PathBuf>, String> {
    let mut args = std::env::args().skip(1);
    let mut out = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => match args.next() {
                Some(path) => out = Some(std::path::PathBuf::from(path)),
                None => return Err("--out needs a path".to_string()),
            },
            "-h" | "--help" => {
                println!("usage: layout-probe [--out <file>]");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(out)
}

#[cfg(windows)]
mod abi;

#[cfg(windows)]
mod callback;

#[cfg(windows)]
mod windows;
