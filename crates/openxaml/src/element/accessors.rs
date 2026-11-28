//! Element accessors that map a property descriptor onto a typed value.
//!
//! Kept out of `element.rs` so the type and its invariants are one file and the getters are
//! another; a child module reaches the private fields the same way the parent does.

use crate::element::Element;
use crate::property::{self, Property};
use crate::value::{Orientation, Value};

impl Element {
    /// Record that this element's `property` comes from the control's `source` property.
    pub(crate) fn bind_template(&mut self, property: &'static Property, source: String) {
        self.bindings.push((property.name, source));
    }

    /// Resolve a template's `{TemplateBinding X}` markers against the control the template
    /// was instantiated for: the body says `Panel_Background = Control_Background`, and the
    /// instance is where `Control_Background` has a value.
    pub(crate) fn bind_from(&mut self, source: &Element) {
        let bindings = std::mem::take(&mut self.bindings);
        for (target, name) in bindings {
            // The binding names a property by its declaring type: `Control_Background`.
            let member = name
                .rsplit_once(['_', '.'])
                .map_or(name.as_str(), |(_, tail)| tail);
            let Some(read) = property::lookup(member) else {
                continue;
            };
            let Some(write) = property::lookup(target) else {
                continue;
            };
            let value = source.get(read).clone();
            let _ = self.set(write, value);
        }
        for child in &mut self.children {
            child.bind_from(source);
        }
    }

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
        /*
         * A selector bar's items run across: its template puts them in a repeater the runtime
         * lays out horizontally, and the page sets no orientation on the bar itself. The
         * default is checked on the local value, because `get` would hand back Orientation's
         * own default and never reach this.
         */
        if self.kind() == crate::element::Kind::SelectorBar && self.local("Orientation").is_none() {
            return Orientation::Horizontal;
        }
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

    /// The picture an `Image` names.
    pub fn image_source(&self) -> &str {
        match self.get(&property::SOURCE) {
            Value::Text(text) => text,
            _ => "",
        }
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
