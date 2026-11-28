//! Control templates, taken from the framework's own theme dictionary.
//!
//! A page asks for `Button`; what a Button *is* lives in a `Style` inside
//! `Microsoft.UI.Xaml.Controls.pri`: the style's setters name `Control_Template`, the value
//! is a `ControlTemplate`, its `FrameworkTemplate_Template` is a deferred stream, and that
//! stream's root is the template body. This module walks that chain once and keeps the bodies
//! by type, so the loader can give a control the shape the framework ships.

use std::collections::HashMap;

use xbf::model::{Element as XbfElement, Value};

/// Template bodies by simple type name, e.g. `Button`.
#[derive(Default)]
pub struct Theme {
    bodies: HashMap<String, XbfElement>,
}

impl Theme {
    /// No templates: every control is an empty box, which is the state before this exists.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Read every style in a compiled theme dictionary and keep the template bodies.
    pub fn from_dictionary(file: &xbf::Xbf) -> Self {
        let mut theme = Self::default();
        let Ok(model) = file.model() else {
            return theme;
        };
        for root in &model.roots {
            theme.walk(file, root);
        }
        theme
    }

    /// Take another dictionary's bodies, keeping the ones already known: the first
    /// dictionary that names a type is the one that wins, as it does in a merged
    /// `ResourceDictionary`.
    pub fn merge(&mut self, other: Theme) {
        for (type_name, body) in other.bodies {
            self.bodies.entry(type_name).or_insert(body);
        }
    }

    /// The body for a type, by its full or simple name.
    pub fn body_for(&self, type_name: &str) -> Option<&XbfElement> {
        self.bodies.get(&simple(type_name))
    }

    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    fn walk(&mut self, file: &xbf::Xbf, element: &XbfElement) {
        if element.type_name.ends_with("Style")
            && let Some(target) = type_property(element, "TargetType")
            && let Some(body) = template_body(file, element)
        {
            self.bodies.entry(target).or_insert(body);
        }
        for child in &element.children {
            self.walk(file, child);
        }
    }
}

fn simple(type_name: &str) -> String {
    type_name
        .rsplit('.')
        .next()
        .unwrap_or(type_name)
        .to_string()
}

fn type_property(element: &XbfElement, ends_with: &str) -> Option<String> {
    element
        .properties
        .iter()
        .find(|property| property.name.ends_with(ends_with))
        .and_then(|property| match &property.value {
            Value::Type { name, .. } => Some(simple(name)),
            _ => None,
        })
}

/// Setters -> `ControlTemplate` -> its deferred body stream.
fn template_body(file: &xbf::Xbf, style: &XbfElement) -> Option<XbfElement> {
    let values = file.stream_values(style.custom_substream? as usize).ok()?;
    for value in values {
        let Value::Element(object) = &value.value else {
            continue;
        };
        if !object.type_name.ends_with("ControlTemplate") {
            continue;
        }
        let stream = object
            .properties
            .iter()
            .find_map(|property| match &property.value {
                Value::Deferred { substream } => Some(*substream as usize),
                _ => None,
            })?;
        let body = file.substream(stream).ok()?;
        return body.roots.first().cloned();
    }
    None
}
