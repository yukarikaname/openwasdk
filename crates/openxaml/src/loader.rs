//! Bridge from the XBF element model to the element tree this crate lays out.
//!
//! The XBF crate reads the file and resolves names; this module makes the two
//! models meet. It accepts only what [`crate::support`] lists. A type, property
//! or value it cannot convert returns an error that names it, because that name
//! is the work item for the next control.

use std::fmt;

use crate::element::{Element, Kind, Value};
use crate::support;

mod property;
mod value;

use property::load_property_with;

/// Why an XBF tree could not become an element tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// The registry has no constructor for this element type.
    UnsupportedType { name: String },
    /// The owner's table entry does not name this property.
    UnsupportedProperty { element: String, name: String },
    /// The property is known, but the XBF value is not convertible. A resource
    /// reference lands here until there is a resource map.
    UnsupportedValue { element: String, property: String },
}

impl fmt::Display for LoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedType { name } => {
                write!(formatter, "unsupported element type `{name}`")
            }
            Self::UnsupportedProperty { element, name } => {
                write!(formatter, "unsupported property `{name}` on `{element}`")
            }
            Self::UnsupportedValue { element, property } => {
                write!(
                    formatter,
                    "unsupported value for `{property}` on `{element}`"
                )
            }
        }
    }
}

impl std::error::Error for LoadError {}

/// The bodies of the application's own controls, by simple type name.
///
/// A \`<controls:ControlExample>\` in a page names a class whose body is another
/// \`.xbf\`. Without it the loader would have to guess that an unknown type is a
/// container; with it the body is the control, which is what XAML does.
#[derive(Default)]
pub struct Controls {
    bodies: std::collections::HashMap<String, xbf::Model>,
    /*
     * Application controls whose shipped XAML holds no tree: a ResourceDictionary
     * with templates, or nothing the writer could keep. Their content is built in
     * code, so a preview draws the box the page asked for and nothing inside it.
     */
    unrealized: std::collections::HashSet<String>,
    /// Control templates from the framework's theme dictionary: what a control is, when the
    /// page only says which control it wants.
    pub theme: crate::theme::Theme,
}

impl Controls {
    /// Register one control body. The key is the simple name a page refers to.
    pub fn insert(&mut self, name: &str, body: xbf::Model) {
        self.bodies.insert(name.to_ascii_lowercase(), body);
    }

    /// Use the framework's theme dictionary for the controls that have a template.
    pub fn set_theme(&mut self, theme: crate::theme::Theme) {
        self.theme = theme;
    }

    /// Register a control that has no visual tree to load.
    pub fn insert_unrealized(&mut self, name: &str) {
        self.unrealized.insert(name.to_ascii_lowercase());
    }

    fn body(&self, type_name: &str) -> Option<&xbf::Model> {
        let simple = type_name.rsplit('.').next().unwrap_or(type_name);
        self.bodies.get(&simple.to_ascii_lowercase())
    }

    fn has_unrealized(&self, type_name: &str) -> bool {
        let simple = type_name.rsplit('.').next().unwrap_or(type_name);
        self.unrealized.contains(&simple.to_ascii_lowercase())
    }
}

/// A control body may not contain itself; past this depth the page is refusing to
/// terminate rather than nesting.
const MAX_DEPTH: usize = 8;

/// Load every root of a model.
pub fn load(model: &xbf::Model) -> Result<Vec<Element>, LoadError> {
    load_with(model, &Controls::default())
}

/// Load every root of a model, resolving the application's own controls.
pub fn load_with(model: &xbf::Model, controls: &Controls) -> Result<Vec<Element>, LoadError> {
    model
        .roots
        .iter()
        .map(|root| load_element(root, controls, 0))
        .collect()
}

/// Load one XBF element and its subtree.
pub fn load_root(source: &xbf::Element) -> Result<Element, LoadError> {
    load_element(source, &Controls::default(), 0)
}

fn load_element(
    source: &xbf::Element,
    controls: &Controls,
    depth: usize,
) -> Result<Element, LoadError> {
    load_with_tolerance(source, controls, depth, false)
}

fn load_with_tolerance(
    source: &xbf::Element,
    controls: &Controls,
    depth: usize,
    tolerant: bool,
) -> Result<Element, LoadError> {
    /*
     * XBF's deferred-object marker: the writer leaves a placeholder whose object is realized
     * later by the deferred-element table. This loader has no such table, so the placeholder
     * becomes an empty host -- the template around it still renders, which is better than
     * losing the whole control.
     */
    if source.type_name.ends_with("DeferredElement")
        && let Some(support) = support::by_kind(Kind::Unrealized)
    {
        return build(source, support, controls, depth);
    }
    let Some(support) = support::lookup(&source.type_name) else {
        return load_body(source, controls, depth);
    };
    build_with(source, support, controls, depth, tolerant)
}

/// A type outside the standard table, realized from the body its application shipped.
fn load_body(
    source: &xbf::Element,
    controls: &Controls,
    depth: usize,
) -> Result<Element, LoadError> {
    let unsupported = LoadError::UnsupportedType {
        name: source.type_name.clone(),
    };
    if depth >= MAX_DEPTH {
        return Err(unsupported);
    }
    if controls.has_unrealized(&source.type_name) {
        return unrealized(source, controls, depth);
    }
    let body = controls.body(&source.type_name).ok_or(unsupported)?;
    let Some(root) = body.roots.first() else {
        return Err(LoadError::UnsupportedType {
            name: source.type_name.clone(),
        });
    };
    let mut element = load_element(root, controls, depth + 1)?;
    // What the page set on the instance wins over the body's defaults, as it does
    // when the runtime applies a local value to a control's template output.
    let support = support::lookup(root.type_name.as_str());
    let Some(support) = support else {
        return Ok(element);
    };
    for property in &source.properties {
        load_property_with(
            &mut element,
            support,
            &source.type_name,
            property,
            controls,
            depth,
        )?;
    }
    for child in &source.children {
        element.push(load_element(child, controls, depth + 1)?);
    }
    Ok(element)
}

/// A control the application built in code: an empty box of the size the page asked for.
fn unrealized(
    source: &xbf::Element,
    controls: &Controls,
    depth: usize,
) -> Result<Element, LoadError> {
    let Some(support) = support::by_kind(Element::new(Kind::Unrealized).kind()) else {
        return Err(LoadError::UnsupportedType {
            name: source.type_name.clone(),
        });
    };
    build(source, support, controls, depth)
}

fn build(
    source: &xbf::Element,
    support: &'static support::TypeSupport,
    controls: &Controls,
    depth: usize,
) -> Result<Element, LoadError> {
    build_with(source, support, controls, depth, false)
}

fn build_with(
    source: &xbf::Element,
    support: &'static support::TypeSupport,
    controls: &Controls,
    depth: usize,
    tolerant: bool,
) -> Result<Element, LoadError> {
    let mut element = Element::new(support.kind);
    for property in &source.properties {
        /*
         * A framework template sets properties this renderer does not convert -- focus
         * visuals, scroll state, a template part's flags. A page is held to the table; a
         * template gets the rest of its tree built with those properties dropped, so one
         * unknown member cannot cost the whole control.
         */
        let result = load_property_with(
            &mut element,
            support,
            &source.type_name,
            property,
            controls,
            depth,
        );
        if let Err(error) = result {
            if tolerant && matches!(error, LoadError::UnsupportedProperty { .. }) {
                continue;
            }
            return Err(error);
        }
    }
    // A dictionary's entries are resources (Double, String, brushes); nothing in this
    // renderer resolves them, so they are not built as elements.
    if support.kind.is_non_visual() {
        return Ok(element);
    }
    for child in &source.children {
        // The tolerance travels with the tree: a template's children are as forgiving as the
        // template itself.
        let child = if tolerant {
            load_with_tolerance(child, controls, depth, true)?
        } else {
            load_element(child, controls, depth)?
        };
        element.push(child);
    }
    /*
     * A control that the page does not give a tree gets the framework's: the theme
     * dictionary's `ControlTemplate` for its type, with the instance's own content placed
     * where the template asks for it. The style's other setters come first -- a control's
     * background, foreground and corner radius live there, not in the page's markup, and the
     * template binds to them.
     */
    if let Some(style) = controls.theme.style_for(&source.type_name) {
        apply_setters(&mut element, &style.setters, &controls.theme);
        if let Some(body) = style.body.as_ref() {
            // The control's own content belongs inside the template, not beside it.
            let content = element.take_children();
            /*
             * A template body exercises more of the surface than the page does -- visual
             * states, template bindings, a control we have not met. When it cannot be built
             * the page keeps its empty box: one control without chrome is a worse picture, a
             * page that fails to load is no picture.
             */
            match load_with_tolerance(body, controls, depth + 1, true) {
                Ok(mut template) => {
                    // The body's {TemplateBinding X} markers name the control's properties.
                    template.bind_from(&element);
                    place_content(&mut template, content);
                    if std::env::var_os("OPENCOMP_TRACE").is_some() {
                        eprintln!("openxaml: template for {} instantiated", source.type_name);
                    }
                    element.set_children(vec![template]);
                }
                Err(error) => {
                    if std::env::var_os("OPENCOMP_TRACE").is_some() {
                        eprintln!(
                            "openxaml: template for {} left empty: {error}",
                            source.type_name
                        );
                    }
                }
            }
        }
    }
    Ok(element)
}

/// The style's own values: theme colours through the palette, typed constants as they are.
fn apply_setters(
    element: &mut Element,
    setters: &[(String, crate::theme::SetterValue)],
    theme: &crate::theme::Theme,
) {
    for (name, value) in setters {
        let member = name
            .rsplit_once(['_', '.'])
            .map_or(name.as_str(), |(_, tail)| tail);
        if member == "Template" {
            continue;
        }
        let Some(descriptor) = crate::property::lookup(member) else {
            continue;
        };
        let converted = match value {
            crate::theme::SetterValue::Key(key)
                if descriptor.value_type == crate::property::ValueType::SolidColorBrush =>
            {
                crate::theme::palette::color(key).map(Value::Color)
            }
            crate::theme::SetterValue::Key(key) => theme
                .resource(key)
                .and_then(|value| constant_value(value, descriptor.value_type)),
            crate::theme::SetterValue::Constant(constant) => {
                constant_value(constant, descriptor.value_type)
            }
            _ => None,
        };
        if let Some(converted) = converted {
            let _ = element.set(descriptor, converted);
        }
    }
}

/// Put a control's content where its template asks for it: the `ContentPresenter`.
fn place_content(template: &mut Element, content: Vec<Element>) {
    if content.is_empty() {
        return;
    }
    match find_kind(template, Kind::ContentPresenter) {
        Some(presenter) => presenter.push_all(content),
        None => template.push_all(content),
    }
}

fn find_kind(element: &mut Element, kind: Kind) -> Option<&mut Element> {
    if element.kind() == kind {
        return Some(element);
    }
    for child in element.children_mut() {
        if let Some(found) = find_kind(child, kind) {
            return Some(found);
        }
    }
    None
}

/// A style's typed constant, in the shape the property's descriptor declares.
fn constant_value(
    constant: &xbf::model::Value,
    expected: crate::property::ValueType,
) -> Option<Value> {
    use crate::element::{CornerRadius, Thickness};
    use crate::property::ValueType;
    use xbf::model::Value as Wire;
    match (expected, constant) {
        (ValueType::Thickness, Wire::Thickness([l, t, r, b])) => {
            Some(Value::Thickness(Thickness::new(*l, *t, *r, *b)))
        }
        (ValueType::Thickness, Wire::Float(all)) => Some(Value::Thickness(Thickness::all(*all))),
        (ValueType::CornerRadius, Wire::Thickness([l, t, r, b])) => {
            Some(Value::CornerRadius(CornerRadius::new(*l, *t, *r, *b)))
        }
        (ValueType::CornerRadius, Wire::Float(all)) => {
            Some(Value::CornerRadius(CornerRadius::all(*all)))
        }
        (ValueType::Double, Wire::Float(value)) => Some(Value::Number(*value)),
        (ValueType::Double, Wire::Signed(value)) => Some(Value::Number(*value as f32)),
        _ => None,
    }
}
