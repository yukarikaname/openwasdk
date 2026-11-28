/*
 * Typed element model built from the flat object-writer node stream.
 *
 * The node stream is a pre-order serialization of the same operations the
 * XAML object writer performs.  PushScope* opens a scope and the matching
 * PopScope/EndInitPopScope/EndInitProvideValuePopScope closes it.  Object
 * creation nodes attach an element to the container of the enclosing scope;
 * PushScopeGetValue attaches the objects it contains to the property on the
 * element that was current when the scope opened.
 *
 * Only the node kinds whose meaning can be represented without a WinRT type
 * system are mapped.  Everything else fails with a ModelError that carries
 * the substream, node index and node kind; nothing is skipped silently.
 */

use crate::{Xbf, XbfError};

mod builder;
mod convert;
mod frames;
mod json;
mod resolve;
mod text;

/* XML namespace used by x: directives and markup directive properties. */
pub(super) const XAML_NAMESPACE: &str = "http://schemas.microsoft.com/winfx/2006/xaml";

/*
 * One XAML object.  A resource dictionary keeps its key in an x:Key property
 * on the resource element, matching how the source XAML spells it.
 */
#[derive(Clone, Debug)]
pub struct Element {
    pub type_name: String,
    pub namespace: String,
    pub properties: Vec<Property>,
    pub children: Vec<Element>,
    pub custom: Option<String>,
    /// The stream a style's setters (or a template's body) live in, when the writer put
    /// them out of line.
    pub custom_substream: Option<u32>,
    /// A style's setters, when the element carries `Style_v*` custom data.
    pub setters: Vec<crate::node::StyleSetter>,
}

#[derive(Clone, Debug)]
pub struct Property {
    pub name: String,
    pub namespace: String,
    pub value: Value,
}

/*
 * A persisted constant, a resolved type or property reference, a resource
 * reference, or an inline element.  Values that came from a property element
 * scope arrive as Element, or as Elements when the scope held more than one.
 */
#[derive(Clone, Debug)]
pub enum Value {
    None,
    Bool(bool),
    Float(f32),
    Signed(i32),
    String(String),
    Thickness([f32; 4]),
    GridLength { unit: u32, value: f32 },
    Color(u32),
    Null,
    Enum { type_name: String, value: u32 },
    Type { name: String, namespace: String },
    Property { name: String, namespace: String },
    TemplateBinding { name: String, namespace: String },
    Resource { theme: bool, key: String },
    /* A property whose value lives in a substream that is realized on first use. */
    Deferred { substream: u32 },
    /* A property assigned the result of a markup extension that was not invoked. */
    Extension { name: String },
    Element(Box<Element>),
    Elements(Vec<Element>),
}

#[derive(Clone, Debug)]
pub struct Model {
    pub roots: Vec<Element>,
}

impl Model {
    /* Indented Type.property = value rendering, one line per property. */
    pub fn to_text(&self) -> String {
        text::render(self)
    }

    /* Hand written JSON for the same tree; the crate stays dependency free. */
    pub fn to_json(&self) -> String {
        json::render(self)
    }
}

/* Build one substream as its own model: a style's setters and a template's body live in
 * a referenced stream, and a consumer that wants them asks for that stream by index. */
pub(crate) fn build_stream(file: &Xbf, index: usize) -> Result<Model, XbfError> {
    let mut built = builder::Builder::new(file, index).run()?;
    let mut visited = vec![index];
    resolve::resolve_references(file, &mut built, &mut visited)?;
    resolve::assign_resource_keys(file, &mut built.raw);
    let roots = built
        .roots
        .iter()
        .map(|&root| resolve::materialize(&built.raw, root))
        .collect();
    Ok(Model { roots })
}

/* The property values one stream sets, in node order, when the stream has no element of
 * its own (a style's setter values are the style's). */
pub(crate) fn stream_values(file: &Xbf, index: usize) -> Result<Vec<Property>, XbfError> {
    let built = builder::Builder::new_with_container(file, index).run()?;
    Ok(built
        .raw
        .first()
        .map(|raw| raw.properties.clone())
        .unwrap_or_default())
}

/* Build the model for a parsed file.  Stream 0 is the root object. */
pub(crate) fn build(file: &Xbf) -> Result<Model, XbfError> {
    let mut built = builder::Builder::new(file, 0).run()?;
    let mut visited = vec![0usize];
    resolve::resolve_references(file, &mut built, &mut visited)?;
    resolve::assign_resource_keys(file, &mut built.raw);
    let roots = built
        .roots
        .iter()
        .map(|&root| resolve::materialize(&built.raw, root))
        .collect();
    Ok(Model { roots })
}
