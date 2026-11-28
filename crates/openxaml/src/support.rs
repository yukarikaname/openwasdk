//! The table of what the loader accepts.
//!
//! The loader has no second list. A type is supported when it is here, and a
//! property is supported when its owner's entry names it. Each entry names the
//! same [`crate::property::Property`] descriptor the element model uses, so a
//! type cannot drift from the registry. Adding a control is one entry; every
//! other type produces an error that names what is missing, which is the only
//! signal we get about what to port next.

use crate::element::Kind;
use crate::property::Property;

pub(crate) struct TypeSupport {
    pub(crate) kind: Kind,
    pub(crate) properties: &'static [&'static Property],
}

mod table;

use table::TABLE;

impl TypeSupport {
    pub(crate) fn property(&self, name: &str) -> Option<&'static Property> {
        self.properties
            .iter()
            .copied()
            .find(|property| property.name == name)
    }
}

/*
 * One entry per type. The macro writes the descriptor references once, into
 * both the conversion table and the list `properties` returns, so the two
 * cannot disagree.
 */
/// Look a type up by its short or its namespaced WinRT name.
pub(crate) fn lookup(type_name: &str) -> Option<&'static TypeSupport> {
    TABLE
        .iter()
        .find(|entry| Kind::from_type_name(type_name) == Some(entry.kind))
}

/// The entry for a kind the loader constructs itself, such as an unrealized control.
pub fn by_kind(kind: Kind) -> Option<&'static TypeSupport> {
    TABLE.iter().find(|entry| entry.kind == kind)
}

/// Whether the loader has a constructor for this element type.
pub fn supports(type_name: &str) -> bool {
    lookup(type_name).is_some()
}

/// The properties the loader converts for this element type. An unknown type
/// has no accepted properties.
pub fn properties(type_name: &str) -> &'static [&'static Property] {
    match lookup(type_name) {
        Some(entry) => entry.properties,
        None => &[],
    }
}

#[cfg(test)]
pub(crate) fn all_property_names() -> impl Iterator<Item = &'static str> {
    TABLE
        .iter()
        .flat_map(|entry| entry.properties.iter().map(|property| property.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::{LoadError, load_root};
    use crate::property::ValueType;
    use xbf::model::{Element, Property as WireProperty, Value};

    const CONTROLS: &str = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";

    fn source(type_name: &str, properties: Vec<WireProperty>) -> Element {
        Element {
            type_name: type_name.to_string(),
            namespace: CONTROLS.to_string(),
            properties,
            children: Vec::new(),
            custom: None,
            custom_substream: None,
            setters: Vec::new(),
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
                vec![WireProperty {
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
                    vec![WireProperty {
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
            assert_eq!(properties(entry.kind.type_name()), entry.properties);
        }
    }

    #[test]
    fn every_table_property_is_accepted() {
        for entry in TABLE {
            for property in entry.properties {
                let source = source(
                    entry.kind.type_name(),
                    vec![WireProperty {
                        name: property.name.to_string(),
                        namespace: CONTROLS.to_string(),
                        value: sample(property.name, property.value_type),
                    }],
                );
                let loaded = load_root(&source).expect("every table property is accepted");
                assert!(
                    loaded.local(property.name).is_some(),
                    "{} was not stored",
                    property.name
                );
            }
        }
    }

    #[test]
    fn a_type_outside_the_table_is_rejected() {
        /* A name no table will ever carry, so the test keeps testing the rejection. */
        let error = load_root(&source("NoSuchControl", Vec::new()))
            .expect_err("NoSuchControl is not in the table");
        assert_eq!(
            error,
            LoadError::UnsupportedType {
                name: "NoSuchControl".to_string()
            }
        );
        assert!(!supports("NoSuchControl"));
        assert!(properties("NoSuchControl").is_empty());
    }
}
