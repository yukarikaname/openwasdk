//! Dependency-property descriptors and the effective-value rule.
//!
//! A property is a static descriptor, not a string in a map. The descriptor
//! carries the type a value must have, the value a read returns when nothing
//! was set, and whether the property inherits down the tree. The registry
//! below names every property the support table accepts, so the loader and the
//! element model cannot disagree about a property's type or default.
//!
//! The effective value follows the base-value chain in
//! `dxaml/xcp/components/DependencyObject/PropertySystem.cpp`
//! (`CDependencyObject::EvaluateBaseValue` and `CDependencyObject::GetValueInherited`):
//!
//! 1. the local value set on the element
//! 2. the value inherited from the nearest ancestor that set one
//! 3. the property default
//!
//! Styles, theme resources, bindings and animations are later sources in the
//! upstream chain. This layer has none of them yet, so the three rules above
//! are the whole chain. `EvaluateBaseValue` evaluates style and built-in style
//! between the local value and the default; those sources are absent rather
//! than reordered.

use std::borrow::Cow;
use std::fmt;

use crate::value::{
    CornerRadius, HorizontalAlignment, Length, Orientation, Thickness, Value, VerticalAlignment,
    Visibility,
};

/// A property name, exactly as XAML spells it. Attached properties keep their
/// owner prefix, for example `Grid.Row`.
pub type PropertyId = &'static str;

/// The value shape a property expects. A value of another shape is rejected by
/// [`crate::element::Element::set`] instead of being converted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueType {
    Double,
    Length,
    Thickness,
    CornerRadius,
    SolidColorBrush,
    GridDefinitions,
    Text,
    Bool,
    TypeName,
    /// An enumeration; the name is the WinRT type without its namespace.
    Enum(&'static str),
}

/// One dependency property. The descriptor is static so an identity is a
/// pointer and a name lookup cannot invent a second descriptor by accident.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Property {
    pub name: PropertyId,
    pub value_type: ValueType,
    pub default: &'static Value,
    pub inherits: bool,
}

impl Property {
    pub const fn new(
        name: PropertyId,
        value_type: ValueType,
        default: &'static Value,
        inherits: bool,
    ) -> Self {
        Self {
            name,
            value_type,
            default,
            inherits,
        }
    }

    /// Whether a value may be stored for this property. The check is exact:
    /// a number does not become a length and text does not become a value.
    pub fn accepts(&self, value: &Value) -> bool {
        value.value_type() == self.value_type
    }
}

impl Value {
    /// The shape a stored value has. Used for the set-time type check.
    pub fn value_type(&self) -> ValueType {
        match self {
            Self::Number(_) => ValueType::Double,
            Self::Length(_) => ValueType::Length,
            Self::Thickness(_) => ValueType::Thickness,
            Self::CornerRadius(_) => ValueType::CornerRadius,
            Self::HorizontalAlignment(_) => ValueType::Enum("HorizontalAlignment"),
            Self::VerticalAlignment(_) => ValueType::Enum("VerticalAlignment"),
            Self::Orientation(_) => ValueType::Enum("Orientation"),
            Self::Visibility(_) => ValueType::Enum("Visibility"),
            Self::Color(_) => ValueType::SolidColorBrush,
            Self::Text(_) => ValueType::Text,
            Self::GridDefinitions(_) => ValueType::GridDefinitions,
            Self::Bool(_) => ValueType::Bool,
            Self::TypeName(_) => ValueType::TypeName,
        }
    }
}

/// Why a value did not go into the store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertyError {
    pub property: PropertyId,
    pub expected: ValueType,
    pub actual: ValueType,
}

impl PropertyError {
    pub fn new(property: &Property, value: &Value) -> Self {
        Self {
            property: property.name,
            expected: property.value_type,
            actual: value.value_type(),
        }
    }
}

impl fmt::Display for PropertyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "property `{}` expects {:?}, found {:?}",
            self.property, self.expected, self.actual
        )
    }
}

impl std::error::Error for PropertyError {}

/// `Control.FontSize` in the theme.
pub const DEFAULT_FONT_SIZE: f32 = 14.0;

pub static WIDTH: Property = Property::new(
    "Width",
    ValueType::Length,
    &Value::Length(Length::Auto),
    false,
);
pub static HEIGHT: Property = Property::new(
    "Height",
    ValueType::Length,
    &Value::Length(Length::Auto),
    false,
);
pub static MARGIN: Property = Property::new(
    "Margin",
    ValueType::Thickness,
    &Value::Thickness(Thickness::ZERO),
    false,
);
pub static PADDING: Property = Property::new(
    "Padding",
    ValueType::Thickness,
    &Value::Thickness(Thickness::ZERO),
    false,
);
pub static HORIZONTAL_ALIGNMENT: Property = Property::new(
    "HorizontalAlignment",
    ValueType::Enum("HorizontalAlignment"),
    &Value::HorizontalAlignment(HorizontalAlignment::Stretch),
    false,
);
pub static VERTICAL_ALIGNMENT: Property = Property::new(
    "VerticalAlignment",
    ValueType::Enum("VerticalAlignment"),
    &Value::VerticalAlignment(VerticalAlignment::Stretch),
    false,
);
pub static BACKGROUND: Property = Property::new(
    "Background",
    ValueType::SolidColorBrush,
    &Value::Color([0.0; 4]),
    false,
);
pub static ORIENTATION: Property = Property::new(
    "Orientation",
    ValueType::Enum("Orientation"),
    &Value::Orientation(Orientation::Vertical),
    false,
);
/// `UIElement.Visibility`. A collapsed element is not measured, takes no space
/// and does not advance a `StackPanel` cursor or count toward its `Spacing`.
pub static VISIBILITY: Property = Property::new(
    "Visibility",
    ValueType::Enum("Visibility"),
    &Value::Visibility(Visibility::Visible),
    false,
);
pub static SPACING: Property =
    Property::new("Spacing", ValueType::Double, &Value::Number(0.0), false);
/// `Grid.RowSpacing`, between rows only.
pub static GRID_ROW_SPACING: Property = Property::new(
    "Grid.RowSpacing",
    ValueType::Double,
    &Value::Number(0.0),
    false,
);
/// `Grid.ColumnSpacing`, between columns only.
pub static GRID_COLUMN_SPACING: Property = Property::new(
    "Grid.ColumnSpacing",
    ValueType::Double,
    &Value::Number(0.0),
    false,
);
pub static TEXT: Property = Property::new(
    "Text",
    ValueType::Text,
    &Value::Text(Cow::Borrowed("")),
    false,
);
/// `Image.Source`: the picture an `Image` shows, as the page wrote it.
pub static SOURCE: Property = Property::new(
    "Source",
    ValueType::Text,
    &Value::Text(Cow::Borrowed("")),
    false,
);

/// The one inherited property in the current table. A value set on a container
/// reaches every descendant that has no local value of its own.
pub static FONT_SIZE: Property = Property::new(
    "FontSize",
    ValueType::Double,
    &Value::Number(DEFAULT_FONT_SIZE),
    true,
);
pub static BORDER_BRUSH: Property = Property::new(
    "BorderBrush",
    ValueType::SolidColorBrush,
    &Value::Color([0.0; 4]),
    false,
);
pub static BORDER_THICKNESS: Property = Property::new(
    "BorderThickness",
    ValueType::Thickness,
    &Value::Thickness(Thickness::ZERO),
    false,
);
pub static CORNER_RADIUS: Property = Property::new(
    "CornerRadius",
    ValueType::CornerRadius,
    &Value::CornerRadius(CornerRadius::ZERO),
    false,
);
pub static GRID_ROW: Property =
    Property::new("Grid.Row", ValueType::Double, &Value::Number(0.0), false);
pub static GRID_COLUMN: Property =
    Property::new("Grid.Column", ValueType::Double, &Value::Number(0.0), false);
/// A child crosses this many rows; one row is the default.
pub static GRID_ROW_SPAN: Property = Property::new(
    "Grid.RowSpan",
    ValueType::Double,
    &Value::Number(1.0),
    false,
);
/// A child crosses this many columns; one column is the default.
pub static GRID_COLUMN_SPAN: Property = Property::new(
    "Grid.ColumnSpan",
    ValueType::Double,
    &Value::Number(1.0),
    false,
);
pub static GRID_ROWS: Property = Property::new(
    "Grid.RowDefinitions",
    ValueType::GridDefinitions,
    &Value::GridDefinitions(Cow::Borrowed(&[])),
    false,
);
pub static GRID_COLUMNS: Property = Property::new(
    "Grid.ColumnDefinitions",
    ValueType::GridDefinitions,
    &Value::GridDefinitions(Cow::Borrowed(&[])),
    false,
);

/// Every descriptor the support table names. The loader indexes this list by
/// the name in the table; a property missing here is a table error, not a
/// silent default.
/// `UIElement.Opacity`: 0..1, default 1, multiplied down the subtree (WinUI:
/// learn.microsoft.com/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.uielement.opacity).
pub static OPACITY: Property =
    Property::new("Opacity", ValueType::Double, &Value::Number(1.0), false);

/// `Canvas.Left` / `Canvas.Top`: the attached offsets a Canvas child asks for.
pub static CANVAS_LEFT: Property =
    Property::new("Canvas.Left", ValueType::Double, &Value::Number(0.0), false);
pub static CANVAS_TOP: Property =
    Property::new("Canvas.Top", ValueType::Double, &Value::Number(0.0), false);

pub static PROPERTIES: &[&Property] = &[
    &SOURCE,
    &CANVAS_LEFT,
    &CANVAS_TOP,
    &OPACITY,
    &WIDTH,
    &HEIGHT,
    &MARGIN,
    &PADDING,
    &HORIZONTAL_ALIGNMENT,
    &VERTICAL_ALIGNMENT,
    &BACKGROUND,
    &ORIENTATION,
    &VISIBILITY,
    &SPACING,
    &GRID_ROW_SPACING,
    &GRID_COLUMN_SPACING,
    &TEXT,
    &FONT_SIZE,
    &BORDER_BRUSH,
    &BORDER_THICKNESS,
    &CORNER_RADIUS,
    &GRID_ROW,
    &GRID_COLUMN,
    &GRID_ROW_SPAN,
    &GRID_COLUMN_SPAN,
    &GRID_ROWS,
    &GRID_COLUMNS,
];

/// Properties whose value travels down the tree. Kept as a list so inheritance
/// is a property flag, not a match arm in `Element`.
pub static INHERITED: &[&Property] = &[&FONT_SIZE];

/// Look a descriptor up by its XAML name.
pub fn lookup(name: &str) -> Option<&'static Property> {
    PROPERTIES
        .iter()
        .copied()
        .find(|property| property.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_support_table_name_has_a_descriptor() {
        for name in crate::support::all_property_names() {
            assert!(lookup(name).is_some(), "`{name}` has no descriptor");
        }
    }

    #[test]
    fn every_descriptor_default_has_the_declared_type() {
        for property in PROPERTIES {
            assert!(
                property.accepts(property.default),
                "the default for `{}` does not match its type",
                property.name
            );
        }
    }

    #[test]
    fn inherited_properties_are_in_the_registry() {
        for property in INHERITED {
            assert!(PROPERTIES.iter().any(|entry| entry.name == property.name));
            assert!(property.inherits);
        }
    }
}
