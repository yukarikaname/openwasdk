/*
 * Scope stack for the object-writer node stream.
 *
 * A frame is opened by PushScope* and closed by PopScope, EndInitPopScope or
 * EndInitProvideValuePopScope.  Raw elements live in a flat arena so a child
 * scope can attach an element to its parent without moving the subtree.
 */

use super::resolve::{convert, materialize};
use super::{Element, Property, Value, XAML_NAMESPACE};
use crate::node::{Node, NodeKind, PersistedNode, ResourceKey, Value as WireValue};
use crate::{Xbf, XbfError};

pub(super) struct RawElement {
    pub(super) type_name: String,
    pub(super) namespace: String,
    pub(super) properties: Vec<Property>,
    pub(super) children: Vec<usize>,
    pub(super) custom: Option<String>,
    pub(super) custom_substream: Option<u32>,
    pub(super) explicit_keys: Vec<ResourceKey>,
    pub(super) implicit_keys: Vec<ResourceKey>,
    pub(super) explicit_key: Option<String>,
    /* Absolute substream node offset of the creation node. */
    pub(super) source_offset: u32,
}

pub(super) struct Built {
    pub(super) raw: Vec<RawElement>,
    pub(super) roots: Vec<usize>,
}

struct Frame {
    element: Option<usize>,
    property: Option<PendingProperty>,
    namespaces: Vec<(String, String)>,
}

struct PendingProperty {
    owner: usize,
    name: String,
    namespace: String,
    values: Vec<usize>,
}

pub(super) struct Builder<'a> {
    pub(super) file: &'a Xbf,
    stream: usize,
    pub(super) raw: Vec<RawElement>,
    roots: Vec<usize>,
    frames: Vec<Frame>,
    base_namespaces: Vec<(String, String)>,
    values: Vec<Value>,
}

impl<'a> Builder<'a> {
    pub(super) fn new(file: &'a Xbf, stream: usize) -> Self {
        Builder {
            file,
            stream,
            raw: Vec::new(),
            roots: Vec::new(),
            frames: Vec::new(),
            base_namespaces: Vec::new(),
            values: Vec::new(),
        }
    }

    pub(super) fn run(mut self) -> Result<Built, XbfError> {
        let Some(node_stream) = self.file.streams.get(self.stream) else {
            return Err(self.error(0, "substream", "referenced substream does not exist"));
        };
        for (index, node) in node_stream.nodes.iter().enumerate() {
            if matches!(node.kind, NodeKind::EndOfStream) {
                break;
            }
            self.apply(index, node)?;
        }
        self.finish_frames();

        Ok(Built {
            raw: self.raw,
            roots: self.roots,
        })
    }

    fn error(&self, index: usize, kind: &'static str, reason: &'static str) -> XbfError {
        XbfError::ModelError {
            stream: self.stream,
            index,
            kind,
            reason,
        }
    }

    fn apply(&mut self, index: usize, node: &Node) -> Result<(), XbfError> {
        let kind = node.name();
        match &node.kind {
            /* Stack manipulation. */
            NodeKind::PushScope => {
                self.frames.push(Frame {
                    element: None,
                    property: None,
                    namespaces: Vec::new(),
                });
            }
            NodeKind::PopScope
            | NodeKind::EndInitPopScope
            | NodeKind::EndInitProvideValuePopScope => self.pop(index, kind)?,

            /* Scopes that also spend a namespace declaration. */
            NodeKind::AddNamespace { namespace, prefix } => {
                let uri = self.file.namespace_label(namespace);
                self.add_namespace(prefix, uri);
            }
            NodeKind::PushScopeAddNamespace { namespace, prefix } => {
                let uri = self.file.namespace_label(namespace);
                self.frames.push(Frame {
                    element: None,
                    property: None,
                    namespaces: vec![(prefix.clone(), uri)],
                });
            }

            /* Deferred constant stack, consumed by SetValue. */
            NodeKind::PushConstant { value } => self.values.push(convert(value)),
            NodeKind::SetValue { property } => {
                let Some(value) = self.values.pop() else {
                    return Err(self.error(index, kind, "SetValue has no pushed constant"));
                };
                let default = self.default_namespace();
                let (name, namespace) = self.resolve_property(*property, &default);
                self.set_property(index, kind, name, namespace, value)?;
            }

            /* Object creation. */
            NodeKind::CreateTypeBeginInit { ty } => {
                self.create_object(*ty, None, node.offset);
            }
            NodeKind::CreateTypeWithConstantBeginInit { ty, value }
            | NodeKind::CreateTypeWithTypeConvertedConstantBeginInit { ty, value } => {
                self.create_object(*ty, Some(convert(value)), node.offset);
            }
            NodeKind::PushScopeCreateTypeBeginInit { ty } => {
                self.open_object(*ty, None, node.offset);
            }
            NodeKind::PushScopeCreateTypeWithConstantBeginInit { ty, value }
            | NodeKind::PushScopeCreateTypeWithTypeConvertedConstantBeginInit { ty, value } => {
                self.open_object(*ty, Some(convert(value)), node.offset);
            }

            /* Property element scope: objects attach to the element on top. */
            NodeKind::PushScopeGetValue { property } => {
                let Some(owner) = self.current_element() else {
                    return Err(self.error(index, kind, "property scope has no parent element"));
                };
                let default = self.default_namespace();
                let (name, namespace) = self.resolve_property(*property, &default);
                self.frames.push(Frame {
                    element: None,
                    property: Some(PendingProperty {
                        owner,
                        name,
                        namespace,
                        values: Vec::new(),
                    }),
                    namespaces: Vec::new(),
                });
            }

            /* Direct property assignment. */
            NodeKind::SetValueConstant { property, value }
            | NodeKind::SetValueTypeConvertedConstant { property, value } => {
                let default = self.default_namespace();
                let (name, namespace) = self.resolve_property(*property, &default);
                self.set_property(index, kind, name, namespace, convert(value))?;
            }
            NodeKind::SetValueTypeConvertedResolvedType { property, ty } => {
                let default = self.default_namespace();
                let (name, namespace) = self.resolve_property(*property, &default);
                let (type_name, type_namespace) = self.resolve_type(*ty, &default);
                let value = Value::Type {
                    name: type_name,
                    namespace: type_namespace,
                };
                self.set_property(index, kind, name, namespace, value)?;
            }
            NodeKind::SetValueTypeConvertedResolvedProperty { property, value } => {
                let default = self.default_namespace();
                let (name, namespace) = self.resolve_property(*property, &default);
                let (ref_name, ref_namespace) = self.resolve_property(*value, &default);
                let value = Value::Property {
                    name: ref_name,
                    namespace: ref_namespace,
                };
                self.set_property(index, kind, name, namespace, value)?;
            }
            NodeKind::SetValueFromTemplateBinding { property, value } => {
                let default = self.default_namespace();
                let (name, namespace) = self.resolve_property(*property, &default);
                let (ref_name, ref_namespace) = self.resolve_property(*value, &default);
                let value = Value::TemplateBinding {
                    name: ref_name,
                    namespace: ref_namespace,
                };
                self.set_property(index, kind, name, namespace, value)?;
            }
            NodeKind::SetValueFromStaticResource { property, value }
            | NodeKind::SetValueFromThemeResource { property, value } => {
                let theme = matches!(node.kind, NodeKind::SetValueFromThemeResource { .. });
                let default = self.default_namespace();
                let (name, namespace) = self.resolve_property(*property, &default);
                let value = Value::Resource {
                    theme,
                    key: key_of(value),
                };
                self.set_property(index, kind, name, namespace, value)?;
            }

            /* Directives and structural collection operations. */
            NodeKind::SetName { value } => {
                let value = convert(value);
                self.set_directive(index, kind, "x:Name", value)?;
            }
            NodeKind::SetConnectionId { value } => {
                let value = convert(value);
                self.set_directive(index, kind, "x:ConnectionId", value)?;
            }
            NodeKind::AddToDictionaryWithKey { value } => {
                self.set_last_key(index, kind, convert(value))?;
            }
            NodeKind::AddToCollection
            | NodeKind::AddToDictionary
            | NodeKind::CheckPeerType { .. } => {
                /*
                 * Elements are attached when they are created, so these
                 * confirmation nodes need no further change to the tree.
                 */
            }

            /* Custom runtime data. */
            NodeKind::SetCustomRuntimeData { substream, data } => {
                let Some(owner) = self.current_element() else {
                    return Err(self.error(index, kind, "custom data has no current element"));
                };
                let raw = &mut self.raw[owner];
                raw.custom = Some(data.kind_name.to_string());
                raw.custom_substream = Some(*substream);
                raw.explicit_keys = data.explicit_keys.clone();
                raw.implicit_keys = data.implicit_keys.clone();
            }
            NodeKind::EndOfStream => {}

            /*
             * Refused: these need a live type system, resource lookup,
             * markup extension evaluation or conditional XAML evaluation, or
             * they push an operand that no persisted node consumes.  Mapping
             * them to a plausible looking value would be wrong.
             */
            NodeKind::PushResolvedType { .. }
            | NodeKind::PushResolvedProperty { .. }
            | NodeKind::GetResourcePropertyBag { .. }
            | NodeKind::SetDeferredProperty { .. }
            | NodeKind::SetValueFromMarkupExtension { .. }
            | NodeKind::ProvideStaticResourceValue { .. }
            | NodeKind::ProvideThemeResourceValue { .. }
            | NodeKind::BeginConditionalScope { .. }
            | NodeKind::EndConditionalScope => {
                return Err(self.error(index, kind, "node kind has no model mapping"));
            }
        }
        Ok(())
    }

    fn add_namespace(&mut self, prefix: &str, uri: String) {
        let entry = (prefix.to_string(), uri);
        match self.frames.last_mut() {
            Some(frame) => frame.namespaces.push(entry),
            None => self.base_namespaces.push(entry),
        }
    }

    fn namespace(&self, prefix: &str) -> Option<&str> {
        for frame in self.frames.iter().rev() {
            for (candidate, uri) in frame.namespaces.iter().rev() {
                if candidate == prefix {
                    return Some(uri);
                }
            }
        }
        for (candidate, uri) in self.base_namespaces.iter().rev() {
            if candidate == prefix {
                return Some(uri);
            }
        }
        None
    }

    fn default_namespace(&self) -> String {
        self.namespace("").unwrap_or("").to_string()
    }

    /*
     * Attach a fresh element to the container of the innermost enclosing
     * scope.  A property scope collects into its pending property instead.
     */
    fn attach(&mut self, index: usize) {
        for frame in self.frames.iter_mut().rev() {
            if let Some(pending) = frame.property.as_mut() {
                pending.values.push(index);
                return;
            }
            if let Some(parent) = frame.element {
                self.raw[parent].children.push(index);
                return;
            }
        }
        self.roots.push(index);
    }

    fn make_element(&mut self, ty: PersistedNode, value: Option<Value>, offset: u32) -> usize {
        let default = self.default_namespace();
        let (type_name, namespace) = self.resolve_type(ty, &default);
        let mut properties = Vec::new();
        if let Some(value) = value {
            /*
             * PushScopeCreateTypeWithConstant initializes the object itself;
             * there is no property name on the wire, so keep the constant
             * under the conventional Value name.
             */
            properties.push(Property {
                name: "Value".to_string(),
                namespace: namespace.clone(),
                value,
            });
        }
        let index = self.raw.len();
        self.raw.push(RawElement {
            type_name,
            namespace,
            properties,
            children: Vec::new(),
            custom: None,
            custom_substream: None,
            explicit_keys: Vec::new(),
            implicit_keys: Vec::new(),
            explicit_key: None,
            source_offset: offset,
        });
        self.attach(index);
        index
    }

    /* CreateType*: create in the current scope and remember its object. */
    fn create_object(&mut self, ty: PersistedNode, value: Option<Value>, offset: u32) -> usize {
        let index = self.make_element(ty, value, offset);
        if let Some(frame) = self.frames.last_mut()
            && frame.property.is_none()
            && frame.element.is_none()
        {
            frame.element = Some(index);
        }
        index
    }

    /* PushScopeCreateType*: create in the parent scope, then open a scope. */
    fn open_object(&mut self, ty: PersistedNode, value: Option<Value>, offset: u32) {
        let index = self.make_element(ty, value, offset);
        self.frames.push(Frame {
            element: Some(index),
            property: None,
            namespaces: Vec::new(),
        });
    }

    fn current_element(&self) -> Option<usize> {
        self.frames.iter().rev().find_map(|frame| frame.element)
    }

    fn set_property(
        &mut self,
        index: usize,
        kind: &'static str,
        name: String,
        namespace: String,
        value: Value,
    ) -> Result<(), XbfError> {
        let Some(owner) = self.current_element() else {
            return Err(self.error(index, kind, "property has no current element"));
        };
        self.raw[owner].properties.push(Property {
            name,
            namespace,
            value,
        });
        Ok(())
    }

    fn set_directive(
        &mut self,
        index: usize,
        kind: &'static str,
        name: &str,
        value: Value,
    ) -> Result<(), XbfError> {
        let Some(owner) = self.current_element() else {
            return Err(self.error(index, kind, "directive has no current element"));
        };
        self.raw[owner].properties.push(Property {
            name: name.to_string(),
            namespace: XAML_NAMESPACE.to_string(),
            value,
        });
        Ok(())
    }

    fn set_last_key(
        &mut self,
        index: usize,
        kind: &'static str,
        value: Value,
    ) -> Result<(), XbfError> {
        let Some(owner) = self.current_element() else {
            return Err(self.error(index, kind, "dictionary key has no current element"));
        };
        let target = self.raw[owner].children.last().copied().unwrap_or(owner);
        self.raw[target].properties.push(Property {
            name: "x:Key".to_string(),
            namespace: XAML_NAMESPACE.to_string(),
            value,
        });
        Ok(())
    }

    fn pop(&mut self, index: usize, kind: &'static str) -> Result<(), XbfError> {
        let Some(frame) = self.frames.pop() else {
            return Err(self.error(index, kind, "scope closer has no open scope"));
        };
        self.close_frame(frame);
        Ok(())
    }

    /* Close frames left open when the substream ends; fixtures do this. */
    fn finish_frames(&mut self) {
        while let Some(frame) = self.frames.pop() {
            self.close_frame(frame);
        }
    }

    fn close_frame(&mut self, frame: Frame) {
        let Some(pending) = frame.property else {
            return;
        };
        let values: Vec<Element> = pending
            .values
            .iter()
            .map(|&value| materialize(&self.raw, value))
            .collect();
        let mut iter = values.into_iter();
        let value = match (iter.next(), iter.next()) {
            (None, _) => Value::None,
            (Some(one), None) => Value::Element(Box::new(one)),
            (Some(one), Some(two)) => {
                let mut rest = vec![one, two];
                rest.extend(iter);
                Value::Elements(rest)
            }
        };
        self.raw[pending.owner].properties.push(Property {
            name: pending.name,
            namespace: pending.namespace,
            value,
        });
    }
}

fn key_of(value: &WireValue) -> String {
    match value {
        WireValue::SharedString(value) | WireValue::UniqueString(value) => value.clone(),
        other => other.to_string(),
    }
}
