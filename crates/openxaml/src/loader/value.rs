//! The value conversions a property's descriptor asks for: numbers, lengths,
//! thicknesses, brushes, grid tracks, text, enumerations and the resolved names a
//! type-converted constant carries.

use super::*;
use crate::element::{
    CornerRadius, GridLength, GridTrack, HorizontalAlignment, Orientation, Thickness, Value,
    VerticalAlignment, Visibility,
};

pub(super) fn string_value<'a>(
    element: &str,
    property: &'a xbf::Property,
) -> Result<&'a str, LoadError> {
    match &property.value {
        xbf::model::Value::String(value) => Ok(value),
        _ => Err(LoadError::UnsupportedValue {
            element: element.to_string(),
            property: property.name.clone(),
        }),
    }
}

/* A property that XAML spells as a double may arrive as a float or an integer. */
pub(super) fn number(value: &xbf::model::Value) -> Option<f32> {
    match value {
        xbf::model::Value::Float(value) => Some(*value),
        xbf::model::Value::Signed(value) => Some(*value as f32),
        _ => None,
    }
}

pub(super) fn thickness(value: &xbf::model::Value) -> Option<Thickness> {
    match value {
        xbf::model::Value::Thickness([left, top, right, bottom]) => {
            Some(Thickness::new(*left, *top, *right, *bottom))
        }
        xbf::model::Value::Float(all) => Some(Thickness::all(*all)),
        _ => None,
    }
}

/* CornerRadius shares the four-float wire shape with Thickness. */
pub(super) fn corner_radius(value: &xbf::model::Value) -> Option<CornerRadius> {
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
pub(super) fn solid_color_brush(value: &xbf::model::Value) -> Option<[f32; 4]> {
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
pub(super) fn color_from_argb(color: u32) -> [f32; 4] {
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
pub(super) fn definitions(property: &xbf::Property) -> Option<Vec<GridTrack>> {
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

pub(super) fn definition_property(definition: &xbf::Element, name: &str) -> Option<f32> {
    definition
        .properties
        .iter()
        .find(|property| property.name == name)
        .and_then(|property| number(&property.value))
}

pub(super) fn grid_length(value: &xbf::model::Value) -> Option<GridLength> {
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

pub(super) fn text(value: &xbf::model::Value) -> Option<String> {
    match value {
        xbf::model::Value::String(value) => Some(value.clone()),
        _ => None,
    }
}

pub(super) fn boolean(value: &xbf::model::Value) -> Option<bool> {
    match value {
        xbf::model::Value::Bool(value) => Some(*value),
        _ => None,
    }
}

pub(super) fn resolved_type(value: &xbf::model::Value) -> Option<String> {
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
pub(super) fn enum_value(expected: &str, value: &xbf::model::Value) -> Option<Value> {
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
pub(super) fn name_suffix(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

/// The element values a content property holds, in markup order.
pub(super) fn property_elements(
    property: &xbf::Property,
) -> Result<Vec<xbf::model::Element>, LoadError> {
    Ok(match &property.value {
        xbf::model::Value::Element(one) => vec![(**one).clone()],
        xbf::model::Value::Elements(many) => many.clone(),
        _ => Vec::new(),
    })
}
