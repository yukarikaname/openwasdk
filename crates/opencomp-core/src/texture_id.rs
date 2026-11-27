//! The handle a visual uses to name a bitmap.
//!
//! The id is chosen by whoever uploads the pixels, not by the renderer: the compositor
//! already names its surfaces, and a backend that knows the id needs no map of its own.

/// A caller-chosen handle for an uploaded bitmap.
///
/// Zero names the built-in white texture that untextured draws sample, so uploads start
/// at one.
pub type TextureId = u32;

/// The built-in 1x1 white texture.
pub const WHITE: TextureId = 0;
