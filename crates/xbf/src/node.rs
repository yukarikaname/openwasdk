/*
 * Node stream data types.
 *
 * ObjectWriterNodeType.h lists the one-byte kinds and PersistedConstantType
 * (XamlBinaryMetadata.h:218-232) lists the value kinds.  The decoders live in
 * node_wire.rs and node_value.rs.
 */

use crate::metadata::Metadata;
use std::fmt;

#[derive(Clone, Copy, Debug)]
pub struct PersistedNode {
    pub id: u16,
    pub trusted: bool,
}

#[derive(Clone, Debug)]
pub struct Predicate {
    pub ty: PersistedNode,
    pub arguments: String,
}

#[derive(Clone, Debug)]
pub struct CustomData {
    pub kind: u32,
    pub kind_name: &'static str,
    pub names: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum Value {
    None,
    Bool(bool),
    Float(f32),
    Signed(i32),
    SharedString(String),
    Thickness([f32; 4]),
    GridLength { unit: u32, value: f32 },
    Color(u32),
    UniqueString(String),
    NullString,
    Enum { type_index: u16, value: u32 },
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::None => write!(f, "none"),
            Value::Bool(value) => write!(f, "{value}"),
            Value::Float(value) => write!(f, "{value}"),
            Value::Signed(value) => write!(f, "{value}"),
            Value::SharedString(value) | Value::UniqueString(value) => write!(f, "{value:?}"),
            Value::Thickness([left, top, right, bottom]) => {
                write!(f, "thickness({left},{top},{right},{bottom})")
            }
            Value::GridLength { unit, value } => write!(f, "grid({unit},{value})"),
            Value::Color(value) => write!(f, "#{value:08X}"),
            Value::NullString => write!(f, "null"),
            Value::Enum { type_index, value } => write!(f, "enum({type_index}={value})"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    /* Offset relative to the start of this substream. */
    pub offset: u32,
    pub kind: NodeKind,
}

#[derive(Clone, Debug)]
pub enum NodeKind {
    PushScope,
    PopScope,
    AddNamespace {
        namespace: PersistedNode,
        prefix: String,
    },
    PushConstant {
        value: Value,
    },
    PushResolvedType {
        ty: PersistedNode,
    },
    PushResolvedProperty {
        property: PersistedNode,
    },
    SetValue {
        property: PersistedNode,
    },
    AddToCollection,
    AddToDictionary,
    AddToDictionaryWithKey {
        value: Value,
    },
    CheckPeerType {
        type_name: String,
    },
    SetConnectionId {
        value: Value,
    },
    SetName {
        value: Value,
    },
    GetResourcePropertyBag {
        value: Value,
    },
    SetCustomRuntimeData {
        substream: u32,
        data: CustomData,
    },
    SetDeferredProperty {
        property: PersistedNode,
        substream: u32,
        static_resources: Vec<String>,
        theme_resources: Vec<String>,
    },
    PushScopeAddNamespace {
        namespace: PersistedNode,
        prefix: String,
    },
    PushScopeGetValue {
        property: PersistedNode,
    },
    PushScopeCreateTypeBeginInit {
        ty: PersistedNode,
    },
    PushScopeCreateTypeWithConstantBeginInit {
        ty: PersistedNode,
        value: Value,
    },
    PushScopeCreateTypeWithTypeConvertedConstantBeginInit {
        ty: PersistedNode,
        value: Value,
    },
    CreateTypeBeginInit {
        ty: PersistedNode,
    },
    CreateTypeWithConstantBeginInit {
        ty: PersistedNode,
        value: Value,
    },
    CreateTypeWithTypeConvertedConstantBeginInit {
        ty: PersistedNode,
        value: Value,
    },
    SetValueConstant {
        property: PersistedNode,
        value: Value,
    },
    SetValueTypeConvertedConstant {
        property: PersistedNode,
        value: Value,
    },
    SetValueTypeConvertedResolvedProperty {
        property: PersistedNode,
        value: PersistedNode,
    },
    SetValueTypeConvertedResolvedType {
        property: PersistedNode,
        ty: PersistedNode,
    },
    SetValueFromStaticResource {
        property: PersistedNode,
        value: Value,
    },
    SetValueFromTemplateBinding {
        property: PersistedNode,
        value: PersistedNode,
    },
    SetValueFromMarkupExtension {
        property: PersistedNode,
    },
    EndInitPopScope,
    ProvideStaticResourceValue {
        value: Value,
    },
    ProvideThemeResourceValue {
        value: Value,
    },
    SetValueFromThemeResource {
        property: PersistedNode,
        value: Value,
    },
    EndOfStream,
    BeginConditionalScope {
        predicate: Predicate,
    },
    EndConditionalScope,
    EndInitProvideValuePopScope,
}

impl Node {
    /* True for nodes that open a nested scope. */
    pub fn is_opener(&self) -> bool {
        matches!(
            self.kind,
            NodeKind::PushScope
                | NodeKind::PushScopeAddNamespace { .. }
                | NodeKind::PushScopeGetValue { .. }
                | NodeKind::PushScopeCreateTypeBeginInit { .. }
                | NodeKind::PushScopeCreateTypeWithConstantBeginInit { .. }
                | NodeKind::PushScopeCreateTypeWithTypeConvertedConstantBeginInit { .. }
                | NodeKind::BeginConditionalScope { .. }
        )
    }

    /* True for nodes that close a nested scope. */
    pub fn is_closer(&self) -> bool {
        matches!(
            self.kind,
            NodeKind::PopScope
                | NodeKind::EndInitPopScope
                | NodeKind::EndInitProvideValuePopScope
                | NodeKind::EndConditionalScope
        )
    }

    /* Stable short name, handy for logs and for the dump tool. */
    pub fn name(&self) -> &'static str {
        match self.kind {
            NodeKind::PushScope => "PushScope",
            NodeKind::PopScope => "PopScope",
            NodeKind::AddNamespace { .. } => "AddNamespace",
            NodeKind::PushConstant { .. } => "PushConstant",
            NodeKind::PushResolvedType { .. } => "PushResolvedType",
            NodeKind::PushResolvedProperty { .. } => "PushResolvedProperty",
            NodeKind::SetValue { .. } => "SetValue",
            NodeKind::AddToCollection => "AddToCollection",
            NodeKind::AddToDictionary => "AddToDictionary",
            NodeKind::AddToDictionaryWithKey { .. } => "AddToDictionaryWithKey",
            NodeKind::CheckPeerType { .. } => "CheckPeerType",
            NodeKind::SetConnectionId { .. } => "SetConnectionId",
            NodeKind::SetName { .. } => "SetName",
            NodeKind::GetResourcePropertyBag { .. } => "GetResourcePropertyBag",
            NodeKind::SetCustomRuntimeData { .. } => "SetCustomRuntimeData",
            NodeKind::SetDeferredProperty { .. } => "SetDeferredProperty",
            NodeKind::PushScopeAddNamespace { .. } => "PushScopeAddNamespace",
            NodeKind::PushScopeGetValue { .. } => "PushScopeGetValue",
            NodeKind::PushScopeCreateTypeBeginInit { .. } => "PushScopeCreateTypeBeginInit",
            NodeKind::PushScopeCreateTypeWithConstantBeginInit { .. } => {
                "PushScopeCreateTypeWithConstantBeginInit"
            }
            NodeKind::PushScopeCreateTypeWithTypeConvertedConstantBeginInit { .. } => {
                "PushScopeCreateTypeWithTypeConvertedConstantBeginInit"
            }
            NodeKind::CreateTypeBeginInit { .. } => "CreateTypeBeginInit",
            NodeKind::CreateTypeWithConstantBeginInit { .. } => "CreateTypeWithConstantBeginInit",
            NodeKind::CreateTypeWithTypeConvertedConstantBeginInit { .. } => {
                "CreateTypeWithTypeConvertedConstantBeginInit"
            }
            NodeKind::SetValueConstant { .. } => "SetValueConstant",
            NodeKind::SetValueTypeConvertedConstant { .. } => "SetValueTypeConvertedConstant",
            NodeKind::SetValueTypeConvertedResolvedProperty { .. } => {
                "SetValueTypeConvertedResolvedProperty"
            }
            NodeKind::SetValueTypeConvertedResolvedType { .. } => {
                "SetValueTypeConvertedResolvedType"
            }
            NodeKind::SetValueFromStaticResource { .. } => "SetValueFromStaticResource",
            NodeKind::SetValueFromTemplateBinding { .. } => "SetValueFromTemplateBinding",
            NodeKind::SetValueFromMarkupExtension { .. } => "SetValueFromMarkupExtension",
            NodeKind::EndInitPopScope => "EndInitPopScope",
            NodeKind::ProvideStaticResourceValue { .. } => "ProvideStaticResourceValue",
            NodeKind::ProvideThemeResourceValue { .. } => "ProvideThemeResourceValue",
            NodeKind::SetValueFromThemeResource { .. } => "SetValueFromThemeResource",
            NodeKind::EndOfStream => "EndOfStream",
            NodeKind::BeginConditionalScope { .. } => "BeginConditionalScope",
            NodeKind::EndConditionalScope => "EndConditionalScope",
            NodeKind::EndInitProvideValuePopScope => "EndInitProvideValuePopScope",
        }
    }
}

#[derive(Debug)]
pub struct NodeStream {
    pub node_offset: u32,
    pub line_offset: u32,
    pub nodes: Vec<Node>,
}

pub(crate) fn parse(data: &[u8], metadata: &Metadata) -> Result<Vec<NodeStream>, crate::XbfError> {
    crate::node_wire::parse(data, metadata)
}
