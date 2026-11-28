//! Control templates, taken from the framework's own theme dictionary.
//!
//! A page asks for `Button`; what a Button *is* lives in a `Style` inside
//! `Microsoft.UI.Xaml.Controls.pri`: the style's setters name `Control_Template`, the value
//! is a `ControlTemplate`, its `FrameworkTemplate_Template` is a deferred stream, and that
//! stream's root is the template body. This module walks that chain once and keeps the bodies
//! by type, so the loader can give a control the shape the framework ships.

pub mod palette;

use std::collections::HashMap;

use xbf::model::{Element as XbfElement, Value};

/// What a style setter holds: a theme-resource key, or a typed constant.
#[derive(Clone)]
pub enum SetterValue {
    /// A theme resource: the key names a colour in `palette`.
    Key(String),
    /// A scalar, a thickness or a corner radius, as the writer stored it.
    Constant(xbf::model::Value),
    /// Something this loader does not apply (an object, a string, a template).
    Skip,
}

/// A control's implicit style: its template body and the property values it sets.
#[derive(Clone)]
pub struct Style {
    pub body: Option<XbfElement>,
    /// Another style this one starts from (`BasedOn`), by key.
    pub based_on: Option<String>,
    /// Setter property (as the style names it, e.g. `Control_Background`) and its value.
    pub setters: Vec<(String, SetterValue)>,
}

/// Template bodies by simple type name, e.g. `Button`.
#[derive(Default)]
pub struct Theme {
    styles: HashMap<String, Style>,
    /// Styles a page reaches by key, e.g. `Style="{StaticResource BodyTextBlockStyle}"`.
    by_key: HashMap<String, Style>,
    /// Non-colour resources the dictionary defines: `ButtonPadding` is a Thickness, and a
    /// style setter names it by key just like a brush.
    resources: HashMap<String, xbf::model::Value>,
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
        for (type_name, style) in other.styles {
            self.styles.entry(type_name).or_insert(style);
        }
        for (key, value) in other.resources {
            self.resources.entry(key).or_insert(value);
        }
    }

    /// The same, with the other dictionary winning: an application's resources are looked up
    /// before the framework's, so a style the app ships replaces the one the framework ships.
    pub fn merge_over(&mut self, other: Theme) {
        self.styles.extend(other.styles);
        self.by_key.extend(other.by_key);
        self.resources.extend(other.resources);
    }

    /// A resource the dictionary defines by value: a thickness, a length, a radius.
    pub fn resource(&self, key: &str) -> Option<&xbf::model::Value> {
        self.resources.get(key)
    }

    /// The body for a type, by its full or simple name.
    pub fn body_for(&self, type_name: &str) -> Option<&XbfElement> {
        self.style_for(type_name)
            .and_then(|style| style.body.as_ref())
    }

    pub fn style_for(&self, type_name: &str) -> Option<&Style> {
        self.styles.get(&simple(type_name))
    }

    /// The style for a type with its `BasedOn` chain flattened: the base's setters come
    /// first, then the derived ones, and the first body in the chain is the template. An
    /// implicit style often holds nothing but a `BasedOn`, which is why the chain matters.
    pub fn resolved_style(&self, type_name: &str) -> Option<Style> {
        let mut chain = Vec::new();
        let mut current = self.styles.get(&simple(type_name));
        for _ in 0..8 {
            let Some(style) = current else { break };
            chain.push(style);
            current = style
                .based_on
                .as_deref()
                .and_then(|base| self.by_key.get(base));
        }
        if chain.is_empty() {
            return None;
        }
        chain.reverse();
        let mut setters = Vec::new();
        let mut body = None;
        for style in chain {
            setters.extend(style.setters.iter().cloned());
            if body.is_none() {
                body = style.body.clone();
            }
        }
        Some(Style {
            body,
            based_on: None,
            setters,
        })
    }

    /// The setters a style key names, following `BasedOn` so a derived style starts from its
    /// base. This is how `Style="{StaticResource BodyTextBlockStyle}"` reaches a TextBlock.
    pub fn setters_for_key(&self, key: &str) -> Vec<(String, SetterValue)> {
        let mut chain = Vec::new();
        let mut current = self.by_key.get(key);
        for _ in 0..8 {
            let Some(style) = current else { break };
            chain.push(style);
            current = style
                .based_on
                .as_deref()
                .and_then(|base| self.by_key.get(base));
        }
        chain
            .into_iter()
            .rev()
            .flat_map(|style| style.setters.iter().cloned())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.styles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }

    fn walk(&mut self, file: &xbf::Xbf, element: &XbfElement) {
        // `<Thickness x:Key="ButtonPadding"><Thickness.Value>8,4,8,4</Thickness.Value></Thickness>`
        let simple_type = element
            .type_name
            .rsplit('.')
            .next()
            .unwrap_or(&element.type_name);
        if matches!(
            simple_type,
            "Thickness" | "Double" | "CornerRadius" | "GridLength"
        ) {
            let key = element
                .properties
                .iter()
                .find(|property| property.name.contains("Key"))
                .and_then(|property| match &property.value {
                    Value::String(key) => Some(key.clone()),
                    _ => None,
                });
            let value = element
                .properties
                .iter()
                .find(|property| property.name.ends_with("Value"))
                .map(|property| property.value.clone());
            if let (Some(key), Some(value)) = (key, value) {
                self.resources.entry(key).or_insert(value);
            }
        }
        if element.type_name.ends_with("Style")
            && let Some(target) = type_property(element, "TargetType")
        {
            let key = element
                .properties
                .iter()
                .find(|property| property.name.contains("Key"))
                .and_then(|property| match &property.value {
                    Value::String(key) => Some(key.clone()),
                    _ => None,
                });
            let based_on = element
                .properties
                .iter()
                .find(|property| property.name.ends_with("BasedOn"))
                .and_then(|property| match &property.value {
                    Value::Resource { key, .. } => Some(key.clone()),
                    _ => None,
                });
            let style = Style {
                body: template_body(file, element),
                based_on,
                setters: setters(file, element),
            };
            if let Some(key) = key {
                self.by_key.entry(key).or_insert_with(|| style.clone());
            }
            self.styles.entry(target).or_insert(style);
        }
        for child in &element.children {
            self.walk(file, child);
        }
        /*
         * A resource dictionary can hang its entries off a property
         * (`ThemeDictionaries`, `MergedDictionaries`) instead of the child list; both are
         * entries of the same dictionary and both are walked.
         */
        for property in &element.properties {
            match &property.value {
                Value::Element(one) => self.walk(file, one),
                Value::Elements(many) => {
                    for child in many {
                        self.walk(file, child);
                    }
                }
                _ => {}
            }
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

/// The style's setters: the property it names and, for a theme resource, the key its value
/// stream entry carries. The stream holds one node per resource-valued setter, in setter
/// order, which is how a token becomes a key without a second offset table.
fn setters(file: &xbf::Xbf, style: &XbfElement) -> Vec<(String, SetterValue)> {
    let Some(stream) = style.custom_substream else {
        return Vec::new();
    };
    let keys: Vec<Option<String>> = file
        .stream_values(stream as usize)
        .map(|values| {
            values
                .into_iter()
                .map(|value| match value.value {
                    Value::Resource { key, .. } => Some(key),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    let mut next = 0;
    style
        .setters
        .iter()
        .map(|setter| {
            let name = setter.name.clone().unwrap_or_default();
            let value = match &setter.value {
                xbf::StyleSetterValue::Token(_) => {
                    let key = keys.get(next).cloned().flatten();
                    next += 1;
                    match key {
                        Some(key) => SetterValue::Key(key),
                        None => SetterValue::Skip,
                    }
                }
                xbf::StyleSetterValue::Constant(value) => SetterValue::Constant(value.clone()),
                _ => SetterValue::Skip,
            };
            (name, value)
        })
        .collect()
}
