//! `opencomp-client` is the program that makes Gate A concrete.
//!
//! It creates a real window, activates `Compositor`, `ContentIsland` and
//! `DesktopChildSiteBridge` through `opencomp`'s exported `DllGetActivationFactory`, builds a
//! small tree through the generated ABI, and lets the bridge present it. `OPENCOMP_FRAMES`
//! sets the number of message-loop iterations (default 120).
//!
//! The window and the ABI are Windows only, so on any other target the binary only says so.

#[cfg(not(windows))]
fn main() {
    println!("opencomp-client: this client needs Windows; the composition ABI is Windows only");
}

#[cfg(windows)]
fn main() {
    std::process::exit(windows::run());
}

#[cfg(windows)]
mod windows;
