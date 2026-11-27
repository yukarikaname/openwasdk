//! `xaml-probe` asks the official `Microsoft.ui.xaml.dll` what it exports and whether its
//! runtime classes activate. The composition-only host proves the ABI this workspace serves;
//! this probe proves the DLL behind it, which is what Gate B runs under Wine.
//!
//! `XAML_DLL` names the DLL to load. `OPENCOMP_XAML_PROBE_JSON=1` turns the report into one
//! JSON object per line, for CI. The probe activates and reports; it never opens a window and
//! never pumps a message loop.

#[cfg(not(windows))]
fn main() {
    println!("xaml-probe: this probe needs Windows; Microsoft.ui.xaml.dll is Windows only");
}

#[cfg(windows)]
fn main() {
    std::process::exit(windows::run());
}

#[cfg(windows)]
mod xaml_bindings;

#[cfg(windows)]
mod windows;
