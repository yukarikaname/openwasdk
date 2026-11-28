//! Bridge from the XBF element model to the element tree this crate lays out.
//!
//! The XBF crate reads the file and resolves names; this module makes the two
//! models meet. It accepts only what [`crate::support`] lists. A type, property
//! or value it cannot convert returns an error that names it, because that name
//! is the work item for the next control.

use std::fmt;

use crate::element::{
    CornerRadius, Element, GridLength, GridTrack, HorizontalAlignment, Length, Orientation,
    Thickness, Value, VerticalAlignment, Visibility,
};
use crate::property::ValueType;
use crate::support::{self, TypeSupport};

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
}

impl Controls {
    /// Register one control body. The key is the simple name a page refers to.
    pub fn insert(&mut self, name: &str, body: xbf::Model) {
        self.bodies.insert(name.to_ascii_lowercase(), body);
    }

    fn body(&self, type_name: &str) -> Option<&xbf::Model> {
        let simple = type_name.rsplit('.').next().unwrap_or(type_name);
        self.bodies.get(&simple.to_ascii_lowercase())
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
        load_property_with(&mut element, support, &source.type_name, property, controls, depth)?;
    }
    for child in &source.children {
        element.push(load_element(child, controls, depth + 1)?);
    }
    Ok(element)
}

fn build(
    source: &xbf::Element,
    support: &'static support::TypeSupport,
    controls: &Controls,
    depth: usize,
) -> Result<Element, LoadError> {
    let mut element = Element::new(support.kind);
    for property in &source.properties {
        load_property_with(&mut element, support, &source.type_name, property, controls, depth)?;
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

fn load_property_with(
    element: &mut Element,
    support: &'static TypeSupport,
    type_name: &str,
    property: &xbf::Property,
    controls: &Controls,
    depth: usize,
) -> Result<(), LoadError> {
    match property.name.as_str() {
        /* Directives are names, not properties; they stay out of the store. */
        "x:Name" => {
            element.set_name(string_value(type_name, property)?);
            Ok(())
        }
        /* A compiler-assigned connection id: it names a node in traces, not a value. */
        "x:ConnectionId" => Ok(()),
        "x:Key" => {
            element.set_key(string_value(type_name, property)?);
            Ok(())
        }
        name => {
            /*
             * XBF names a property by its declaring type: `FrameworkElement_Width`,
             * `UserControl_Content`, `Control_Background`. What a consumer matches is the
             * member the declaring type introduces, so the prefix goes.
             */
            let member = name.rsplit_once('_').map_or(name, |(_, tail)| tail);
            /*
             * A content property holds the child that a content control shows. It is a
             * child for layout and painting, so it joins the element's children.
             */
            /* FrameworkElement.Name is x:Name under the spelling the writer picked. */
            if member == "Name" {
                element.set_name(string_value(type_name, property)?);
                return Ok(());
            }
            if matches!(member, "Content" | "Child" | "Children") {
                for child in property_elements(property)? {
                    element.push(load_element(&child, controls, depth)?);
                }
                return Ok(());
            }
            /*
             * Properties this renderer has no consumer for: a resource map, a window
             * title, a style's target. Accepting them keeps the page loadable and drops
             * nothing that would have been drawn.
             */
            if matches!(
                member,
                "Resources"
                    | "ThemeDictionaries"
                    | "SystemBackdrop"
                    | "Title"
                    | "TargetType"
                    | "BasedOn"
                    | "Template"
                    | "SuppressHostScrolling"
                    | "IsTabStop"
                    | "Blocks"
                    | "TextWrapping"
                    | "Style"
                    | "Header"
                    | "ToolTip"
                    | "VerticalContentAlignment"
                    | "TextAlignment"
                    | "HeadingLevel"
                    | "HorizontalScrollBarVisibility"
                    | "VerticalScrollBarVisibility"
                    | "TabFocusNavigation"
                    | "PlaceholderText"
                    | "MergedDictionaries"
                    | "Inlines"
                    | "AutomationId"
                    | "CloseButtonText"
                    | "HorizontalScrollMode"
                    | "VerticalScrollMode"
                    | "FontWeight"
                    /* Layout constraints this renderer does not apply yet: dropping them
                     * changes nothing it draws, but the page still measures as if absent. */
                    | "MinHeight"
                    | "MinWidth"
                    | "MaxWidth"
                    | "MaxHeight"
                    | "ZIndex"
                    | "VisualStateGroups"
                    | "IsHitTestVisible"
                    | "TextTrimming"
                    | "Symbol"
                    | "DefaultButton"
            ) {
                return Ok(());
            }
            /*
             * A track collection is a property of the panel that consumes it, and its
             * descriptor says so: `Grid.RowDefinitions`.
             */
            let member = match member {
                "RowDefinitions" => "Grid.RowDefinitions",
                "ColumnDefinitions" => "Grid.ColumnDefinitions",
                // A child of a panel names its cell through the panel's attached property.
                "Row" => "Grid.Row",
                "Column" => "Grid.Column",
                "RowSpacing" => "Grid.RowSpacing",
                "ColumnSpacing" => "Grid.ColumnSpacing",
                "RowSpan" => "Grid.RowSpan",
                "ColumnSpan" => "Grid.ColumnSpan",
                other => other,
            };
            /*
             * A reference -- a resource, a deferred body, a markup extension result -- is
             * not a value this renderer can resolve: it has no resource map and runs no
             * extension. The property stays at its default, which is what the runtime
             * shows until something resolves it.
             */
            if matches!(
                property.value,
                xbf::model::Value::Resource { .. }
                    | xbf::model::Value::Deferred { .. }
                    | xbf::model::Value::Extension { .. }
                    | xbf::model::Value::TemplateBinding { .. }
            ) {
                return Ok(());
            }
            let accepted =
                support
                    .property(member)
                    .ok_or_else(|| LoadError::UnsupportedProperty {
                        element: type_name.to_string(),
                        name: member.to_string(),
                    })?;
            /*
             * A brush that is not a solid colour (gradient, acrylic, image) has no paint
             * in this renderer; leaving the property unset draws the background as nothing.
             */
            if accepted.value_type == ValueType::SolidColorBrush
                && let xbf::model::Value::Element(brush) = &property.value
                && name_suffix(&brush.type_name).ends_with("Brush")
                && name_suffix(&brush.type_name) != "SolidColorBrush"
            {
                return Ok(());
            }
            let value = convert(type_name, property, accepted.value_type)?;
            element
                .set(accepted, value)
                .map_err(|_| LoadError::UnsupportedValue {
                    element: type_name.to_string(),
                    property: name.to_string(),
                })?;
            Ok(())
        }
    }
}

fn convert(
    element: &str,
    property: &xbf::Property,
    expected: ValueType,
) -> Result<Value, LoadError> {
    let converted = match expected {
        ValueType::Double => number(&property.value).map(Value::Number),
        ValueType::Length => {
            number(&property.value).map(|value| Value::Length(Length::Pixels(value)))
        }
        ValueType::Thickness => thickness(&property.value).map(Value::Thickness),
        ValueType::CornerRadius => corner_radius(&property.value).map(Value::CornerRadius),
        ValueType::SolidColorBrush => solid_color_brush(&property.value).map(Value::Color),
        ValueType::GridDefinitions => {
            definitions(property).map(|tracks| Value::GridDefinitions(tracks.into()))
        }
        ValueType::Text => text(&property.value).map(Value::text),
        ValueType::Bool => boolean(&property.value).map(Value::Bool),
        ValueType::TypeName => resolved_type(&property.value).map(Value::TypeName),
        ValueType::Enum(name) => enum_value(name, &property.value),
    };
    converted.ok_or_else(|| LoadError::UnsupportedValue {
        element: element.to_string(),
        property: property.name.clone(),
    })
}

fn string_value<'a>(element: &str, property: &'a xbf::Property) -> Result<&'a str, LoadError> {
    match &property.value {
        xbf::model::Value::String(value) => Ok(value),
        _ => Err(LoadError::UnsupportedValue {
            element: element.to_string(),
            property: property.name.clone(),
        }),
    }
}

/* A property that XAML spells as a double may arrive as a float or an integer. */
fn number(value: &xbf::model::Value) -> Option<f32> {
    match value {
        xbf::model::Value::Float(value) => Some(*value),
        xbf::model::Value::Signed(value) => Some(*value as f32),
        _ => None,
    }
}

fn thickness(value: &xbf::model::Value) -> Option<Thickness> {
    match value {
        xbf::model::Value::Thickness([left, top, right, bottom]) => {
            Some(Thickness::new(*left, *top, *right, *bottom))
        }
        xbf::model::Value::Float(all) => Some(Thickness::all(*all)),
        _ => None,
    }
}

/* CornerRadius shares the four-float wire shape with Thickness. */
fn corner_radius(value: &xbf::model::Value) -> Option<CornerRadius> {
    /*
     * The writer may spell a struct as an object with a Value member
     * (`<CornerRadius>8</CornerRadius>`) instead of an inline constant.
     */
    if let xbf::model::Value::Element(radius) = value {
        if name_suffix(&radius.type_name) == "CornerRadius" {
            let inner = radius
                .properties
                .iter()
                .find(|property| property.name == "Value")
                .map(|property| &property.value);
            return inner.and_then(corner_radius);
        }
        return None;
    }
    match value {
        xbf::model::Value::Thickness([top_left, top_right, bottom_right, bottom_left]) => Some(
            CornerRadius::new(*top_left, *top_right, *bottom_right, *bottom_left),
        ),
        xbf::model::Value::Float(all) => Some(CornerRadius::all(*all)),
        _ => None,
    }
}

/*
 * A brush is an inline element with a Colour. Anything else (a gradient, a
 * resource reference) has no equivalent in the property store yet.
 */
fn solid_color_brush(value: &xbf::model::Value) -> Option<[f32; 4]> {
    let xbf::model::Value::Element(brush) = value else {
        return None;
    };
    if name_suffix(&brush.type_name) != "SolidColorBrush" {
        return None;
    }
    brush
        .properties
        .iter()
        .find(|property| property.name == "Color")
        .and_then(|property| match &property.value {
            xbf::model::Value::Color(color) => Some(color_from_argb(*color)),
            _ => None,
        })
}

/* XBF stores Windows.UI.Color as 0xAARRGGBB. */
fn color_from_argb(color: u32) -> [f32; 4] {
    let bytes = [
        (color >> 24) as u8,
        (color >> 16) as u8,
        (color >> 8) as u8,
        color as u8,
    ];
    opencomp_core::rgba_from_srgb_bytes(bytes)
}

/*
 * Grid.RowDefinitions and Grid.ColumnDefinitions arrive as RowDefinition and
 * ColumnDefinition elements. Each one contributes one track; the owner's
 * property name says whether the track is a height or a width. The definition's
 * Min/Max is carried with the track, because the definition element itself is
 * not part of this layer's model.
 */
fn definitions(property: &xbf::Property) -> Option<Vec<GridTrack>> {
    let (axis, min, max, owner) = if property.name.ends_with("ColumnDefinitions") {
        ("Width", "MinWidth", "MaxWidth", "ColumnDefinition")
    } else {
        ("Height", "MinHeight", "MaxHeight", "RowDefinition")
    };
    let values: Vec<&xbf::Element> = match &property.value {
        xbf::model::Value::Element(definition) => vec![definition],
        xbf::model::Value::Elements(definitions) => definitions.iter().collect(),
        _ => return None,
    };
    values
        .iter()
        .map(|definition| {
            if name_suffix(&definition.type_name) != owner {
                return None;
            }
            let length = definition
                .properties
                .iter()
                .find(|property| property.name == axis)
                .and_then(|property| grid_length(&property.value))
                .unwrap_or_default();
            let min = definition_property(definition, min)
                .map(|value| value.max(0.0))
                .unwrap_or(0.0);
            let max = definition_property(definition, max).unwrap_or(f32::INFINITY);
            Some(GridTrack { length, min, max })
        })
        .collect()
}

fn definition_property(definition: &xbf::Element, name: &str) -> Option<f32> {
    definition
        .properties
        .iter()
        .find(|property| property.name == name)
        .and_then(|property| number(&property.value))
}

fn grid_length(value: &xbf::model::Value) -> Option<GridLength> {
    let xbf::model::Value::GridLength { unit, value } = value else {
        return None;
    };
    match unit {
        0 => Some(GridLength::Auto),
        1 => Some(GridLength::Pixels(*value)),
        2 => Some(GridLength::Star(*value)),
        _ => None,
    }
}

fn text(value: &xbf::model::Value) -> Option<String> {
    match value {
        xbf::model::Value::String(value) => Some(value.clone()),
        _ => None,
    }
}

fn boolean(value: &xbf::model::Value) -> Option<bool> {
    match value {
        xbf::model::Value::Bool(value) => Some(*value),
        _ => None,
    }
}

fn resolved_type(value: &xbf::model::Value) -> Option<String> {
    match value {
        xbf::model::Value::Type { name, .. } => Some(name.clone()),
        _ => None,
    }
}

/*
 * WinRT enumeration values are stable: Orientation is Vertical=0, Horizontal=1;
 * Visibility is Visible=0, Collapsed=1; the alignments are Left/Top=0,
 * Center=1, Right/Bottom=2, Stretch=3.
 */
fn enum_value(expected: &str, value: &xbf::model::Value) -> Option<Value> {
    let xbf::model::Value::Enum { type_name, value } = value else {
        return None;
    };
    if name_suffix(type_name) != expected {
        return None;
    }
    match (expected, *value) {
        ("Orientation", 0) => Some(Value::Orientation(Orientation::Vertical)),
        ("Orientation", 1) => Some(Value::Orientation(Orientation::Horizontal)),
        ("Visibility", 0) => Some(Value::Visibility(Visibility::Visible)),
        ("Visibility", 1) => Some(Value::Visibility(Visibility::Collapsed)),
        ("HorizontalAlignment", 0) => Some(Value::HorizontalAlignment(HorizontalAlignment::Left)),
        ("HorizontalAlignment", 1) => Some(Value::HorizontalAlignment(HorizontalAlignment::Center)),
        ("HorizontalAlignment", 2) => Some(Value::HorizontalAlignment(HorizontalAlignment::Right)),
        ("HorizontalAlignment", 3) => {
            Some(Value::HorizontalAlignment(HorizontalAlignment::Stretch))
        }
        ("VerticalAlignment", 0) => Some(Value::VerticalAlignment(VerticalAlignment::Top)),
        ("VerticalAlignment", 1) => Some(Value::VerticalAlignment(VerticalAlignment::Center)),
        ("VerticalAlignment", 2) => Some(Value::VerticalAlignment(VerticalAlignment::Bottom)),
        ("VerticalAlignment", 3) => Some(Value::VerticalAlignment(VerticalAlignment::Stretch)),
        _ => None,
    }
}

/* A metadata type is fully qualified; the stable table is not. */
fn name_suffix(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

/// The element values a content property holds, in markup order.
fn property_elements(property: &xbf::Property) -> Result<Vec<xbf::model::Element>, LoadError> {
    Ok(match &property.value {
        xbf::model::Value::Element(one) => vec![(**one).clone()],
        xbf::model::Value::Elements(many) => many.clone(),
        _ => Vec::new(),
    })
}
