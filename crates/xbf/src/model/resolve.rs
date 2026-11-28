/*
 * Name resolution, deferred substream merging and materialization.
 *
 * Type and property ids are resolved the same way the node reader labels
 * them: the stable tables for trusted ids, the file's metadata otherwise.
 * Resource dictionary keys carry node stream offsets, so they are matched to
 * elements by the offset recorded at creation time.
 */

use super::builder::{Builder, Built, RawElement};
use super::{Element, Property, Value, XAML_NAMESPACE};
use crate::metadata::{
    PROPERTY_FLAG_IMPLICIT, PROPERTY_FLAG_MARKUP_DIRECTIVE, TYPE_FLAG_MARKUP_DIRECTIVE,
};
use crate::node::{PersistedNode, ResourceKey, Value as WireValue};
use crate::{Xbf, XbfError, stable};

impl Builder<'_> {
    pub(super) fn resolve_type(&self, node: PersistedNode, default: &str) -> (String, String) {
        if node.trusted {
            let name = match stable::type_name(node.id) {
                Some(name) => name.to_string(),
                None => format!("stable-type#{}", node.id),
            };
            return (name, default.to_string());
        }

        let index = node.id as u32;
        let name = match self.file.metadata.type_full_name(index) {
            Some(name) => name,
            None => format!("type#{}", node.id),
        };
        let markup = self
            .file
            .metadata
            .types
            .get(index as usize)
            .is_some_and(|entry| entry.flags & TYPE_FLAG_MARKUP_DIRECTIVE != 0);
        let namespace = if markup {
            XAML_NAMESPACE.to_string()
        } else {
            self.file
                .metadata
                .type_namespace_name(index)
                .unwrap_or(default)
                .to_string()
        };
        (name, namespace)
    }

    pub(super) fn resolve_property(&self, node: PersistedNode, default: &str) -> (String, String) {
        if node.trusted {
            let name = match stable::property_name(node.id) {
                Some(name) => name.to_string(),
                None => format!("stable-property#{}", node.id),
            };
            return (name, default.to_string());
        }

        let index = node.id as u32;
        let name = match self.file.metadata.property_full_name(index) {
            Some(name) => name,
            None => format!("property#{}", node.id),
        };
        let entry = self.file.metadata.properties.get(index as usize);
        let markup = entry.is_some_and(|entry| entry.flags & PROPERTY_FLAG_MARKUP_DIRECTIVE != 0);
        let implicit = entry.is_some_and(|entry| entry.flags & PROPERTY_FLAG_IMPLICIT != 0);
        let namespace = if markup {
            XAML_NAMESPACE.to_string()
        } else if implicit {
            default.to_string()
        } else {
            self.file
                .metadata
                .property_namespace(index)
                .unwrap_or(default)
                .to_string()
        };
        (name, namespace)
    }
}

/*
 * A ResourceDictionary custom blob lives on the dictionary element but its
 * items are materialized from the referenced substream.  Merge those items
 * into the dictionary and let the keys from the blob name them.
 */
pub(super) fn resolve_references(
    file: &Xbf,
    built: &mut Built,
    visited: &mut Vec<usize>,
) -> Result<(), XbfError> {
    let mut index = 0;
    while index < built.raw.len() {
        let is_dictionary = built.raw[index]
            .custom
            .as_deref()
            .is_some_and(|custom| custom.starts_with("ResourceDictionary"));
        if is_dictionary && let Some(substream) = built.raw[index].custom_substream {
            let substream = substream as usize;
            if !visited.contains(&substream) {
                visited.push(substream);
                let mut sub = Builder::new(file, substream).run()?;
                resolve_references(file, &mut sub, visited)?;
                merge_children(built, index, sub);
            }
        }
        index += 1;
    }
    Ok(())
}

fn merge_children(built: &mut Built, parent: usize, mut sub: Built) {
    let offset = built.raw.len();
    for raw in &mut sub.raw {
        for child in &mut raw.children {
            *child += offset;
        }
    }
    let roots: Vec<usize> = sub.roots.iter().map(|&root| root + offset).collect();
    built.raw.extend(sub.raw);

    let children = match roots.first() {
        Some(&first) if built.raw[first].type_name == "ResourceDictionary" => {
            built.raw[first].children.clone()
        }
        _ => roots,
    };
    built.raw[parent].children.extend(children);
}

/*
 * Every resource key carries the offset of the node that realizes its value,
 * relative to the referenced substream.  Match those offsets against the
 * creation offset recorded on the element; do not rely on stream order.
 */
pub(super) fn assign_resource_keys(file: &Xbf, raw: &mut [RawElement]) {
    for index in 0..raw.len() {
        let Some(custom) = raw[index].custom.as_deref() else {
            continue;
        };
        if !custom.starts_with("ResourceDictionary") {
            continue;
        }
        let Some(substream) = raw[index].custom_substream else {
            continue;
        };
        let Some(stream) = file.streams.get(substream as usize) else {
            continue;
        };
        let keys: Vec<ResourceKey> = raw[index]
            .explicit_keys
            .iter()
            .chain(raw[index].implicit_keys.iter())
            .cloned()
            .collect();
        let children = raw[index].children.clone();
        for key in keys {
            let target = stream.node_offset.saturating_add(key.offset);
            let found = children
                .iter()
                .copied()
                .find(|&child| raw[child].source_offset == target);
            if let Some(child) = found
                && raw[child].explicit_key.is_none()
            {
                raw[child].explicit_key = Some(key.name);
            }
        }
    }
}

pub(super) fn materialize(raw: &[RawElement], index: usize) -> Element {
    let entry = &raw[index];
    let mut properties = entry.properties.clone();
    if let Some(key) = &entry.explicit_key {
        properties.push(Property {
            name: "x:Key".to_string(),
            namespace: XAML_NAMESPACE.to_string(),
            value: Value::String(key.clone()),
        });
    }
    Element {
        type_name: entry.type_name.clone(),
        namespace: entry.namespace.clone(),
        properties,
        children: entry
            .children
            .iter()
            .map(|&child| materialize(raw, child))
            .collect(),
        custom: entry.custom.clone(),
        custom_substream: entry.custom_substream,
    }
}

pub(super) fn convert(value: &WireValue) -> Value {
    match value {
        WireValue::None => Value::None,
        WireValue::Bool(value) => Value::Bool(*value),
        WireValue::Float(value) => Value::Float(*value),
        WireValue::Signed(value) => Value::Signed(*value),
        WireValue::SharedString(value) | WireValue::UniqueString(value) => {
            Value::String(value.clone())
        }
        WireValue::Thickness(value) => Value::Thickness(*value),
        WireValue::GridLength { unit, value } => Value::GridLength {
            unit: *unit,
            value: *value,
        },
        WireValue::Color(value) => Value::Color(*value),
        WireValue::NullString => Value::Null,
        WireValue::Enum { type_index, value } => Value::Enum {
            type_name: match stable::type_name(*type_index) {
                Some(name) => name.to_string(),
                None => format!("stable-type#{type_index}"),
            },
            value: *value,
        },
    }
}
