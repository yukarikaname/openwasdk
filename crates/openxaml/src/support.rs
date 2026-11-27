//! The table of what the loader accepts.
//!
//! The loader has no second list. A type is supported when it is here, and a
//! property is supported when its owner's entry names it. Adding a control is
//! one entry; every other type produces an error that names what is missing,
//! which is the only signal we get about what to port next.

use crate::element::Kind;

/// The value shape a property expects. The shape is part of the table so a
/// conversion cannot store a value the property cannot hold.
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

pub(crate) struct PropertySupport {
    pub(crate) name: &'static str,
    pub(crate) value: ValueType,
}

pub(crate) struct TypeSupport {
    pub(crate) kind: Kind,
    pub(crate) properties: &'static [PropertySupport],
    names: &'static [&'static str],
}

impl TypeSupport {
    pub(crate) fn property(&self, name: &str) -> Option<&'static PropertySupport> {
        self.properties
            .iter()
            .find(|property| property.name == name)
    }
}

/*
 * One entry per type. The macro writes the property names once, into both the
 * description the loader converts from and the list `properties` returns, so
 * the two cannot disagree.
 */
macro_rules! type_support {
    ($kind:expr, $($name:literal => $value:expr),* $(,)?) => {
        TypeSupport {
            kind: $kind,
            properties: &[$(PropertySupport { name: $name, value: $value }),*],
            names: &[$($name),*],
        }
    };
}

static TABLE: &[TypeSupport] = &[
    type_support!(
        Kind::StackPanel,
        "Width" => ValueType::Length,
        "Height" => ValueType::Length,
        "Margin" => ValueType::Thickness,
        "Padding" => ValueType::Thickness,
        "HorizontalAlignment" => ValueType::Enum("HorizontalAlignment"),
        "VerticalAlignment" => ValueType::Enum("VerticalAlignment"),
        "Background" => ValueType::SolidColorBrush,
        "Orientation" => ValueType::Enum("Orientation"),
        "Spacing" => ValueType::Double,
        "Grid.Row" => ValueType::Double,
        "Grid.Column" => ValueType::Double,
    ),
    type_support!(
        Kind::Border,
        "Width" => ValueType::Length,
        "Height" => ValueType::Length,
        "Margin" => ValueType::Thickness,
        "Padding" => ValueType::Thickness,
        "HorizontalAlignment" => ValueType::Enum("HorizontalAlignment"),
        "VerticalAlignment" => ValueType::Enum("VerticalAlignment"),
        "Background" => ValueType::SolidColorBrush,
        "BorderBrush" => ValueType::SolidColorBrush,
        "BorderThickness" => ValueType::Thickness,
        "CornerRadius" => ValueType::CornerRadius,
        "Grid.Row" => ValueType::Double,
        "Grid.Column" => ValueType::Double,
    ),
    type_support!(
        Kind::Grid,
        "Width" => ValueType::Length,
        "Height" => ValueType::Length,
        "Margin" => ValueType::Thickness,
        "Padding" => ValueType::Thickness,
        "HorizontalAlignment" => ValueType::Enum("HorizontalAlignment"),
        "VerticalAlignment" => ValueType::Enum("VerticalAlignment"),
        "Background" => ValueType::SolidColorBrush,
        "Grid.RowDefinitions" => ValueType::GridDefinitions,
        "Grid.ColumnDefinitions" => ValueType::GridDefinitions,
        "Grid.Row" => ValueType::Double,
        "Grid.Column" => ValueType::Double,
    ),
    type_support!(
        Kind::TextBlock,
        "Width" => ValueType::Length,
        "Height" => ValueType::Length,
        "Margin" => ValueType::Thickness,
        "Padding" => ValueType::Thickness,
        "HorizontalAlignment" => ValueType::Enum("HorizontalAlignment"),
        "VerticalAlignment" => ValueType::Enum("VerticalAlignment"),
        "Background" => ValueType::SolidColorBrush,
        "Text" => ValueType::Text,
        "FontSize" => ValueType::Double,
        "Grid.Row" => ValueType::Double,
        "Grid.Column" => ValueType::Double,
    ),
];

/// Look a type up by its short or its namespaced WinRT name.
pub(crate) fn lookup(type_name: &str) -> Option<&'static TypeSupport> {
    TABLE
        .iter()
        .find(|entry| Kind::from_type_name(type_name) == Some(entry.kind))
}

/// Whether the loader has a constructor for this element type.
pub fn supports(type_name: &str) -> bool {
    lookup(type_name).is_some()
}

/// The properties the loader converts for this element type. An unknown type
/// has no accepted properties.
pub fn properties(type_name: &str) -> &'static [&'static str] {
    match lookup(type_name) {
        Some(entry) => entry.names,
        None => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::{LoadError, load_root};
    use xbf::model::{Element, Property, Value};

    const CONTROLS: &str = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";

    fn source(type_name: &str, properties: Vec<Property>) -> Element {
        Element {
            type_name: type_name.to_string(),
            namespace: CONTROLS.to_string(),
            properties,
            children: Vec::new(),
            custom: None,
        }
    }

    /*
     * A wire value the table entry must accept. A value that cannot be built
     * without extra context (a brush is an inline element) is built here too.
     */
    fn sample(name: &str, value: ValueType) -> Value {
        match value {
            ValueType::Double => Value::Float(1.0),
            ValueType::Length => Value::Float(1.0),
            ValueType::Thickness => Value::Thickness([1.0, 2.0, 3.0, 4.0]),
            ValueType::CornerRadius => Value::Thickness([1.0, 2.0, 3.0, 4.0]),
            ValueType::SolidColorBrush => Value::Element(Box::new(source(
                "SolidColorBrush",
                vec![Property {
                    name: "Color".to_string(),
                    namespace: CONTROLS.to_string(),
                    value: Value::Color(0xff10_2030),
                }],
            ))),
            ValueType::GridDefinitions => {
                let (axis, owner) = if name.ends_with("RowDefinitions") {
                    ("Height", "RowDefinition")
                } else {
                    ("Width", "ColumnDefinition")
                };
                Value::Element(Box::new(source(
                    owner,
                    vec![Property {
                        name: axis.to_string(),
                        namespace: CONTROLS.to_string(),
                        value: Value::GridLength {
                            unit: 0,
                            value: 0.0,
                        },
                    }],
                )))
            }
            ValueType::Text => Value::String("text".to_string()),
            ValueType::Bool => Value::Bool(true),
            ValueType::TypeName => Value::Type {
                name: "Button".to_string(),
                namespace: CONTROLS.to_string(),
            },
            ValueType::Enum(name) => Value::Enum {
                type_name: name.to_string(),
                value: 1,
            },
        }
    }

    #[test]
    fn every_table_entry_loads() {
        for entry in TABLE {
            let loaded = load_root(&source(entry.kind.type_name(), Vec::new()))
                .expect("every table entry has a constructor");
            assert_eq!(loaded.kind(), entry.kind);
            assert!(supports(entry.kind.type_name()));
            assert_eq!(properties(entry.kind.type_name()), entry.names);
        }
    }

    #[test]
    fn every_table_property_is_accepted() {
        for entry in TABLE {
            assert_eq!(entry.names.len(), entry.properties.len());
            for (property, name) in entry.properties.iter().zip(entry.names) {
                assert_eq!(property.name, *name);
                let source = source(
                    entry.kind.type_name(),
                    vec![Property {
                        name: property.name.to_string(),
                        namespace: CONTROLS.to_string(),
                        value: sample(property.name, property.value),
                    }],
                );
                let loaded = load_root(&source).expect("every table property is accepted");
                assert!(
                    loaded.get(property.name).is_some(),
                    "{} was not stored",
                    property.name
                );
            }
        }
    }

    #[test]
    fn a_type_outside_the_table_is_rejected() {
        let error = load_root(&source("ResourceDictionary", Vec::new()))
            .expect_err("ResourceDictionary is not in the table");
        assert_eq!(
            error,
            LoadError::UnsupportedType {
                name: "ResourceDictionary".to_string()
            }
        );
        assert!(!supports("ResourceDictionary"));
        assert!(properties("ResourceDictionary").is_empty());
    }
}
