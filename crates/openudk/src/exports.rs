//! The names XAML and the lifted compositor import, minus the six conversions that
//! `lib.rs` implements.
//!
//! Split by subsystem so a group can be implemented without touching another: the
//! window and input groups are the ones whose contracts are knowable today, and the
//! media group is the one that needs a media stack before it means anything.

mod composition;
mod input;
mod media;
mod text;
mod window;

#[cfg(test)]
pub(crate) use window::Theming_TrySetDarkTheme;
