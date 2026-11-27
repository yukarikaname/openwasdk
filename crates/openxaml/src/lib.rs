//! The part of a XAML engine that does not need Microsoft's engine.
//!
//! This crate is the first layer of `openxaml`: an element model with a property
//! store, the measure/arrange pass for the four types an app starts with, and the
//! translation into an `opencomp_core::Visual` tree. It has no WinRT, no GPU and
//! no text stack; parsing XBF lives in the `xbf` crate and [`loader`] adapts its
//! element tree. The ABI shell that will answer `Microsoft.ui.xaml.dll` is a
//! later layer and calls into this one.
//!
//! The element shape is deliberately plain: a type name, a store of named
//! properties and an ordered list of children. The bridge from `crates/xbf` to
//! that shape is [`loader`], and [`support`] is the table of what it accepts; a
//! type or property outside the table fails with an error that names it.
//!
//! Layout rules and the upstream file each came from are documented in
//! [`layout`]. Text measurement is a placeholder, not shaping.

mod compose;
mod element;
mod layout;
mod loader;
mod support;
mod value;

pub use compose::compose;
pub use element::{
    CornerRadius, Element, GridLength, HorizontalAlignment, Kind, Length, Orientation, PropertyId,
    Thickness, Value, VerticalAlignment,
};
pub use layout::{Layout, Size, arrange, measure};
pub use loader::{LoadError, load, load_root};
pub use opencomp_core::Rect;
pub use support::{ValueType, properties, supports};
