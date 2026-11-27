//! `Microsoft.UI.Content` hosting: a `ContentIsland` and the child site bridge that owns it.
//!
//! A composition-only client wraps a visual tree in an island and connects it to a site bridge.
//! This module keeps the graph and hands the root node to the host for the renderer snapshot.
//!
//! The shipped package has no Content DLL of its own: `Microsoft.ui.xaml.dll` carries the
//! activation strings for `ContentIsland` and `DesktopChildSiteBridge` and exports
//! `DllGetActivationFactory`. Serving them from `opencomp` is a deliberate choice for our own
//! composition-only host; a faithful mirror would put them in `openxaml`. The class to DLL
//! mapping lives only in `lib.rs`, so it can move. D3D, swapchains and the message loop are
//!
//! The island owns the tree; the bridge owns the window. They are separate modules
//! because the shipped package serves both from the XAML DLL and a future split would
//! move them together, while the bridge is the half a host talks to.

mod bridge;
mod island;

pub(crate) use bridge::bridge_statics;
pub(crate) use island::island_statics;
pub use island::root_node;
