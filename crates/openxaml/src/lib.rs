//! The part of a XAML engine that does not need Microsoft's engine.
//!
//! This crate is the first layer of `openxaml`: an element model with a property
//! store, the measure/arrange pass for the four types an app starts with, and the
//! translation into an `opencomp_core::Visual` tree. It has no WinRT, no XBF reader,
//! no GPU and no text stack. The ABI shell that will answer
//! `Microsoft.ui.xaml.dll` is a later layer and calls into this one.
//!
//! The element shape is deliberately plain: a type name, a store of named
//! properties and an ordered list of children. `crates/xbf` does not have an
//! element model yet, so the adapter from the XBF node stream is not written;
//! when it lands, `Kind::from_type_name`, `Element::set` and `Element::push`
//! are the three calls it needs.
//!
//! Layout rules and the upstream file each came from are documented in
//! [`layout`]. Text measurement is a placeholder, not shaping.

mod compose;
mod element;
mod layout;

pub use compose::compose;
pub use element::{
    Element, GridLength, HorizontalAlignment, Kind, Length, Orientation, PropertyId, Thickness,
    Value, VerticalAlignment,
};
pub use layout::{Layout, Size, arrange, measure};
pub use opencomp_core::Rect;
