//! Element accessors that map a property descriptor onto a typed value.
//!
//! Kept out of `element.rs` so the type and its invariants are one file and the getters are
//! another; a child module reaches the private fields the same way the parent does.

use crate::element::Element;
use crate::property;
use crate::value::{Orientation, Value};

impl Element {
    /// `UIElement.Opacity`, clamped to 0..1 as the property definition requires.
    pub fn opacity(&self) -> f32 {
        match self.get(&property::OPACITY) {
            Value::Number(value) => value.clamp(0.0, 1.0),
            _ => 1.0,
        }
    }

    pub fn background(&self) -> Option<[f32; 4]> {
        match self.get(&property::BACKGROUND) {
            Value::Color(color) => Some(*color),
            _ => None,
        }
    }

    pub fn set_background(&mut self, color: [f32; 4]) {
        self.store(&property::BACKGROUND, Value::Color(color));
    }

    pub fn with_background(self, color: [f32; 4]) -> Self {
        let mut element = self;
        element.set_background(color);
        element
    }

    pub fn orientation(&self) -> Orientation {
        match self.get(&property::ORIENTATION) {
            Value::Orientation(orientation) => *orientation,
            _ => Orientation::Vertical,
        }
    }

    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.store(&property::ORIENTATION, Value::Orientation(orientation));
    }

    pub fn spacing(&self) -> f32 {
        match self.get(&property::SPACING) {
            Value::Number(spacing) => spacing.max(0.0),
            _ => 0.0,
        }
    }

    pub fn set_spacing(&mut self, spacing: f32) {
        self.store(&property::SPACING, Value::Number(spacing.max(0.0)));
    }

    pub fn text(&self) -> &str {
        match self.get(&property::TEXT) {
            Value::Text(text) => text,
            _ => "",
        }
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.store(&property::TEXT, Value::text(text));
    }

    pub fn font_size(&self) -> f32 {
        match self.get(&property::FONT_SIZE) {
            Value::Number(size) => size.max(0.0),
            _ => property::DEFAULT_FONT_SIZE,
        }
    }
}
