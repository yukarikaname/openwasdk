//! Bridge from the XBF element model to the element tree this crate lays out.
//!
//! The XBF crate reads the file and resolves names; this module makes the two
//! models meet. It accepts only what [`crate::support`] lists. A type, property
//! or value it cannot convert returns an error that names it, because that name
//! is the work item for the next control.

use std::fmt;

use crate::element::{Element, Kind};
use crate::support::{self};

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
}

impl Controls {
    /// Register one control body. The key is the simple name a page refers to.
    pub fn insert(&mut self, name: &str, body: xbf::Model) {
        self.bodies.insert(name.to_ascii_lowercase(), body);
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
    let Some(support) = support::lookup(&source.type_name) else {
        return load_body(source, controls, depth);
    };
    build(source, support, controls, depth)
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
    let mut element = Element::new(support.kind);
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
    // A dictionary's entries are resources (Double, String, brushes); nothing in this
    // renderer resolves them, so they are not built as elements.
    if Element::is_non_visual(support.kind) {
        return Ok(element);
    }
    for child in &source.children {
        element.push(load_element(child, controls, depth)?);
    }
    Ok(element)
}
