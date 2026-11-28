/*
 * Scope stack for the object-writer node stream.
 *
 * A frame is opened by PushScope* and closed by PopScope, EndInitPopScope or
 * EndInitProvideValuePopScope.  Raw elements live in a flat arena so a child
 * scope can attach an element to its parent without moving the subtree.
 */

use super::resolve::convert;
use super::{Property, Value};
use crate::node::{Node, NodeKind, ResourceKey, Value as WireValue};
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

pub(super) struct Frame {
    pub(super) element: Option<usize>,
    pub(super) property: Option<PendingProperty>,
    pub(super) namespaces: Vec<(String, String)>,
}

pub(super) struct PendingProperty {
    pub(super) owner: usize,
    pub(super) name: String,
    pub(super) namespace: String,
    pub(super) values: Vec<usize>,
}

pub(super) struct Builder<'a> {
    pub(super) file: &'a Xbf,
    stream: usize,
    pub(super) raw: Vec<RawElement>,
    pub(super) roots: Vec<usize>,
    pub(super) frames: Vec<Frame>,
    pub(super) base_namespaces: Vec<(String, String)>,
    pub(super) values: Vec<Value>,
    /* The element the last closed scope produced; a dictionary key names it. */
    pub(super) last_closed: Option<usize>,
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
            last_closed: None,
        }
    }

    pub(super) fn run(mut self) -> Result<Built, XbfError> {
        let Some(node_stream) = self.file.streams.get(self.stream) else {
            return Err(self.error(0, "substream", "referenced substream does not exist"));
        };
        let count = node_stream.nodes.len();
        for (index, node) in node_stream.nodes.iter().enumerate() {
            if matches!(node.kind, NodeKind::EndOfStream) {
                if index + 1 != count {
                    return Err(self.error(
                        index,
                        node.name(),
                        "nodes follow the end-of-stream marker",
                    ));
                }
                break;
            }
            self.apply(index, node)?;
        }
        /*
         * GenXbf leaves the document scope open; the substream boundary is its
         * closer.  Anything else is a cut or a corrupted scope node, and
         * closing it silently would turn that into a partial tree.
         */
        if self.frames.len() > 1 {
            return Err(self.error(
                count.saturating_sub(1),
                "end of substream",
                "substream ends with more than one open scope",
            ));
        }
        if let Some(frame) = self.frames.first() {
            if frame.property.is_some() {
                return Err(self.error(
                    count.saturating_sub(1),
                    "end of substream",
                    "substream ends inside a property scope",
                ));
            }
            /*
             * A dictionary substream opens with a collection scope and may hold no entry
             * at all; the parent stream still owns the element it collects into.
             */
            if frame.element.is_none() && self.stream == 0 {
                return Err(self.error(
                    count.saturating_sub(1),
                    "end of substream",
                    "substream ends before an object is created",
                ));
            }
        }

        Ok(Built {
            raw: self.raw,
            roots: self.roots,
        })
    }

    pub(super) fn error(&self, index: usize, kind: &'static str, reason: &'static str) -> XbfError {
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
            NodeKind::PopScope => self.pop(index, kind)?,
            /*
             * EndInitPopScope closes an object that the next node takes: SetValue assigns
             * it, AddToCollection adds it. Keep it on the operand stack for that node.
             */
            NodeKind::EndInitPopScope | NodeKind::EndInitProvideValuePopScope => {
                self.pop_object(index, kind)?
            }

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

            /*
             * Operand stack, consumed by SetValue.  Resolved types and
             * properties are operands too: the object writer pushes them and
             * SetValue assigns the popped value, exactly like a constant.
             */
            NodeKind::PushConstant { value } => self.values.push(convert(value)),
            NodeKind::PushResolvedType { ty } => {
                let default = self.default_namespace();
                let (name, namespace) = self.resolve_type(*ty, &default);
                self.values.push(Value::Type { name, namespace });
            }
            NodeKind::PushResolvedProperty { property } => {
                let default = self.default_namespace();
                let (name, namespace) = self.resolve_property(*property, &default);
                self.values.push(Value::Property { name, namespace });
            }
            NodeKind::SetValue { property } => {
                let Some(value) = self.values.pop() else {
                    return Err(self.error(index, kind, "SetValue has no pushed operand"));
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
                let default = self.default_namespace();
                let (name, namespace) = self.resolve_property(*property, &default);
                let Some(owner) = self.current_element() else {
                    /*
                     * A substream continues a collection whose owner is an element in
                     * the parent stream: the dictionary substream repeats the property
                     * node but carries no reference back. Its entries become the roots
                     * of this stream, which is what the merge into the dictionary
                     * expects. Stream 0 has no such excuse.
                     */
                    if self.stream > 0 {
                        self.frames.push(Frame {
                            element: None,
                            property: None,
                            namespaces: Vec::new(),
                        });
                        return Ok(());
                    }
                    return Err(self.error(index, kind, "property scope has no parent element"));
                };
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
            NodeKind::SetValueConstant { property, value } => {
                let default = self.default_namespace();
                let (name, namespace) = self.resolve_property(*property, &default);
                self.set_property(index, kind, name, namespace, convert(value))?;
            }
            NodeKind::SetValueTypeConvertedConstant { property, value } => {
                let default = self.default_namespace();
                let (name, namespace) = self.resolve_property(*property, &default);
                self.set_property(index, kind, name, namespace, converted(value))?;
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
             * Refused: each one needs context that the persisted stream does
             * not carry.  Mapping them to a plausible looking value would be
             * wrong, so the reason names the missing piece.
             */

            /*
             * The constant is the key into a property bag the metadata
             * provider builds for a resource dictionary.  A consumer would
             * need that provider API, not just the key.
             */
            NodeKind::GetResourcePropertyBag { .. } => {
                return Err(self.error(
                    index,
                    kind,
                    "needs a metadata provider resource property bag",
                ));
            }

            /*
             * The referenced substream holds the real value, but the property
             * is realized lazily by a deferred element.  A consumer would need
             * a deferred-element table and realization on first use.
             */
            NodeKind::SetDeferredProperty {
                property,
                substream,
                ..
            } => {
                /*
                 * The property's value is an element tree in another substream, realized on
                 * first use. Keep the reference: attaching it here would lay a template out
                 * as a child, which it is not.
                 */
                let default = self.default_namespace();
                let (name, namespace) = self.resolve_property(*property, &default);
                let value = Value::Deferred {
                    substream: *substream,
                };
                self.set_property(index, kind, name, namespace, value)?;
            }

            /*
             * The markup extension was created by earlier nodes; this node
             * invokes ProvideValue on it.  A consumer would need the live
             * extension instance and the result it produces.
             */
            NodeKind::SetValueFromMarkupExtension { property } => {
                /*
                 * Earlier nodes built the extension object; this node assigns what its
                 * ProvideValue would return. Without invoking it, the honest value is a
                 * marker naming the extension, so a consumer leaves the property unset
                 * instead of pretending it holds the extension itself.
                 */
                let default = self.default_namespace();
                let (name, namespace) = self.resolve_property(*property, &default);
                let extension = match self.values.pop() {
                    Some(Value::Element(extension)) => extension.type_name.clone(),
                    _ => "markup extension".to_string(),
                };
                self.set_property(
                    index,
                    kind,
                    name,
                    namespace,
                    Value::Extension { name: extension },
                )?;
            }

            /*
             * The constant is a resource key.  A consumer would need a resource
             * map to resolve it and a property scope that accepts a scalar,
             * not only the child elements the scope collects today.
             */
            /*
             * A resource reference is a value, not a lookup: it stays unresolved until the
             * consumer has a resource map, which is exactly what SetValue assigns.
             */
            NodeKind::ProvideStaticResourceValue { value } => {
                self.values.push(Value::Resource {
                    theme: false,
                    key: key_of(value),
                });
            }
            NodeKind::ProvideThemeResourceValue { value } => {
                self.values.push(Value::Resource {
                    theme: true,
                    key: key_of(value),
                });
            }

            /*
             * The predicate decides whether the nodes inside the scope exist
             * at all.  A consumer would need XamlPredicateService to evaluate
             * it before the enclosed nodes can be modelled.
             */
            NodeKind::BeginConditionalScope { .. } => {
                return Err(self.error(
                    index,
                    kind,
                    "needs predicate evaluation to decide whether the scoped nodes exist",
                ));
            }
            NodeKind::EndConditionalScope => {
                return Err(self.error(
                    index,
                    kind,
                    "needs the matching conditional scope's decision",
                ));
            }
        }
        Ok(())
    }
}

/*
 * A type-converted constant is the text form of the property's type: "8" for a
 * CornerRadius, "8,8,0,0" for its four corners. The wire carries no other shape
 * information, so one number becomes a scalar and four become a thickness.
 */
fn converted(value: &WireValue) -> Value {
    let resolved = convert(value);
    let Value::String(text) = &resolved else {
        return resolved;
    };
    let parts: Vec<f32> = text
        .split(',')
        .filter_map(|part| part.trim().parse().ok())
        .collect();
    match parts.len() {
        1 => Value::Float(parts[0]),
        4 => Value::Thickness([parts[0], parts[1], parts[2], parts[3]]),
        _ => resolved,
    }
}

fn key_of(value: &WireValue) -> String {
    match value {
        WireValue::SharedString(value) | WireValue::UniqueString(value) => value.clone(),
        other => other.to_string(),
    }
}
