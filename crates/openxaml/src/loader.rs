//! Bridge from the XBF element model to the element tree this crate lays out.
//!
//! The XBF crate reads the file and resolves names; this module makes the two
//! models meet. It accepts only what [`crate::support`] lists. A type, property
//! or value it cannot convert returns an error that names it, because that name
//! is the work item for the next control.

use std::fmt;

use crate::element::{
    CornerRadius, Element, GridLength, HorizontalAlignment, Length, Orientation, Thickness, Value,
    VerticalAlignment,
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

/// Load every root of a model.
pub fn load(model: &xbf::Model) -> Result<Vec<Element>, LoadError> {
    model.roots.iter().map(load_root).collect()
}

/// Load one XBF element and its subtree.
pub fn load_root(source: &xbf::Element) -> Result<Element, LoadError> {
    let support = support::lookup(&source.type_name).ok_or_else(|| LoadError::UnsupportedType {
        name: source.type_name.clone(),
    })?;

    let mut element = Element::new(support.kind);
    for property in &source.properties {
        load_property(&mut element, support, &source.type_name, property)?;
    }
    for child in &source.children {
        element.push(load_root(child)?);
    }
    Ok(element)
}

fn load_property(
    element: &mut Element,
    support: &'static TypeSupport,
    type_name: &str,
    property: &xbf::Property,
) -> Result<(), LoadError> {
    match property.name.as_str() {
        /* Directives are names, not properties; they stay out of the store. */
        "x:Name" => {
            element.set_name(string_value(type_name, property)?);
            Ok(())
        }
        "x:Key" => {
            element.set_key(string_value(type_name, property)?);
            Ok(())
        }
        name => {
            let accepted =
                support
                    .property(name)
                    .ok_or_else(|| LoadError::UnsupportedProperty {
                        element: type_name.to_string(),
                        name: name.to_string(),
                    })?;
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
 * property name says whether the track is a height or a width.
 */
fn definitions(property: &xbf::Property) -> Option<Vec<GridLength>> {
    let (axis, owner) = if property.name.ends_with("ColumnDefinitions") {
        ("Width", "ColumnDefinition")
    } else {
        ("Height", "RowDefinition")
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
            let value = definition
                .properties
                .iter()
                .find(|property| property.name == axis)?;
            grid_length(&value.value)
        })
        .collect()
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
 * the alignments are Left/Top=0, Center=1, Right/Bottom=2, Stretch=3.
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
