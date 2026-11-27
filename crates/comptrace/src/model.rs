//! The WinRT surface a metadata file describes: interfaces with their IIDs and runtime classes
//! with the interfaces they implement.
//!
//! Only the parts a proxy or a stub needs are read. Method signatures, attributes and generic
//! instantiations stay in the metadata until a component asks for them.

use std::fmt;
use std::path::Path;

use windows_metadata::reader::{HasAttributes, Index, Item, TypeCategory};
use windows_metadata::{Type, Value};

/// An interface IID, in the byte order a COM `GUID` uses.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Iid(pub [u8; 16]);

impl fmt::Display for Iid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let b = &self.0;
        write!(
            f,
            "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
            u16::from_be_bytes([b[4], b[5]]),
            u16::from_be_bytes([b[6], b[7]]),
            b[8],
            b[9],
            b[10],
            b[11],
            b[12],
            b[13],
            b[14],
            b[15],
        )
    }
}

impl Iid {
    /// Parse the registry form, with or without braces. Upper case is accepted because logs are
    /// not consistent about it.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().trim_start_matches('{').trim_end_matches('}');
        let groups: Vec<&str> = text.split('-').collect();
        if groups.len() != 5
            || groups[0].len() != 8
            || groups[1].len() != 4
            || groups[2].len() != 4
            || groups[3].len() != 4
            || groups[4].len() != 12
            || !groups
                .iter()
                .all(|group| group.chars().all(|c| c.is_ascii_hexdigit()))
        {
            return None;
        }
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&u32::from_str_radix(groups[0], 16).ok()?.to_be_bytes());
        bytes[4..6].copy_from_slice(&u16::from_str_radix(groups[1], 16).ok()?.to_be_bytes());
        bytes[6..8].copy_from_slice(&u16::from_str_radix(groups[2], 16).ok()?.to_be_bytes());
        let tail = format!("{}{}", groups[3], groups[4]);
        for byte in 0..8 {
            let start = byte * 2;
            bytes[8 + byte] = u8::from_str_radix(&tail[start..start + 2], 16).ok()?;
        }
        Some(Self(bytes))
    }
}

impl fmt::Debug for Iid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// One method, rendered for a report: types are already formatted as a reader expects them.
#[derive(Clone, Debug)]
pub struct Method {
    pub name: String,
    pub returns: String,
    pub params: Vec<String>,
}

impl Method {
    pub fn signature(&self) -> String {
        format!("{} {}({})", self.returns, self.name, self.params.join(", "))
    }
}

#[derive(Clone, Debug)]
pub struct Interface {
    pub namespace: String,
    pub name: String,
    pub iid: Option<Iid>,
    pub methods: Vec<Method>,
}

impl Interface {
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.namespace, self.name)
    }
}

#[derive(Clone, Debug)]
pub struct Class {
    pub namespace: String,
    pub name: String,
    /// Fully qualified names of the interfaces the class implements, in metadata order.
    pub interfaces: Vec<String>,
    /// Interfaces the static methods live on. They are not in `interfaces`: a static interface
    /// is what the activation factory answers `QueryInterface` for.
    pub statics: Vec<String>,
    pub activatable: bool,
}

impl Class {
    pub fn full_name(&self) -> String {
        format!("{}.{}", self.namespace, self.name)
    }
}

#[derive(Default, Debug)]
pub struct Model {
    pub interfaces: Vec<Interface>,
    pub classes: Vec<Class>,
    pub enums: usize,
    pub structs: usize,
    pub delegates: usize,
    pub attributes: usize,
}

impl Model {
    pub fn interface_by_iid(&self, iid: Iid) -> Option<&Interface> {
        self.interfaces
            .iter()
            .find(|interface| interface.iid == Some(iid))
    }

    pub fn namespaces(&self) -> Vec<&str> {
        let mut namespaces: Vec<&str> = self
            .interfaces
            .iter()
            .map(|i| i.namespace.as_str())
            .chain(self.classes.iter().map(|c| c.namespace.as_str()))
            .collect();
        namespaces.sort_unstable();
        namespaces.dedup();
        namespaces
    }
}

/// Read every interface and runtime class in `path`.
pub fn load(path: &Path) -> Result<Model, String> {
    if !path.is_file() {
        return Err(format!("{}: not a file", path.display()));
    }
    let index =
        Index::read(path).ok_or_else(|| format!("{}: not WinRT metadata", path.display()))?;
    let mut model = Model::default();

    for namespace in index.namespaces() {
        for (_, item) in index.namespace_items(namespace) {
            let Item::Type(def) = item else {
                continue;
            };
            match def.category() {
                TypeCategory::Interface => {
                    // A generic interface's methods are written in terms of its parameters; give
                    // the decoder a placeholder for each so it does not index an empty list.
                    let generics: Vec<Type> = (0..def.generic_params().count())
                        .map(|index| Type::Generic(format!("T{index}"), index as u16))
                        .collect();
                    let methods = def
                        .methods()
                        .map(|method| {
                            let signature = method.signature(&generics);
                            Method {
                                name: method.name().to_string(),
                                returns: type_name(&signature.return_type),
                                params: signature.types.iter().map(type_name).collect(),
                            }
                        })
                        .collect();
                    model.interfaces.push(Interface {
                        namespace: def.namespace().to_string(),
                        name: def.name().to_string(),
                        iid: guid(&def),
                        methods,
                    })
                }
                TypeCategory::Class => model.classes.push(Class {
                    namespace: def.namespace().to_string(),
                    name: def.name().to_string(),
                    interfaces: implemented(&def),
                    statics: static_interfaces(&def),
                    activatable: def.find_attribute("ActivatableAttribute").is_some(),
                }),
                TypeCategory::Enum => model.enums += 1,
                TypeCategory::Struct => model.structs += 1,
                TypeCategory::Delegate => model.delegates += 1,
                TypeCategory::Attribute => model.attributes += 1,
            }
        }
    }

    model.interfaces.sort_by(|a, b| {
        (a.namespace.as_str(), a.name.as_str()).cmp(&(b.namespace.as_str(), b.name.as_str()))
    });
    model.classes.sort_by(|a, b| {
        (a.namespace.as_str(), a.name.as_str()).cmp(&(b.namespace.as_str(), b.name.as_str()))
    });
    Ok(model)
}

/// Every attribute on one type, for exploring what the metadata records.
pub fn attributes(path: &Path, full_name: &str) -> Result<Vec<String>, String> {
    let index =
        Index::read(path).ok_or_else(|| format!("{}: not WinRT metadata", path.display()))?;
    for namespace in index.namespaces() {
        for (_, item) in index.namespace_items(namespace) {
            let Item::Type(def) = item else {
                continue;
            };
            let name = format!("{}.{}", def.namespace(), def.name());
            if name != full_name {
                continue;
            }
            let mut out = Vec::new();
            for attribute in def.attributes() {
                let values: Vec<String> = attribute
                    .value()
                    .into_iter()
                    .map(|(name, value)| {
                        if name.is_empty() {
                            format!("{value:?}")
                        } else {
                            format!("{name}={value:?}")
                        }
                    })
                    .collect();
                out.push(format!(
                    "{}.{} ({})",
                    attribute.namespace(),
                    attribute.name(),
                    values.join(", ")
                ));
            }
            return Ok(out);
        }
    }
    Err(format!("{full_name}: not in the metadata"))
}

/// The `GuidAttribute` a WinRT interface carries, decoded from its four fields.
fn guid(def: &windows_metadata::reader::TypeDef<'_>) -> Option<Iid> {
    let attribute = def.find_attribute("GuidAttribute")?;
    let values: Vec<Value> = attribute
        .value()
        .into_iter()
        .map(|(_, value)| value)
        .collect();
    if values.len() != 11 {
        return None;
    }
    let (Value::U32(a), Value::U16(b), Value::U16(c)) = (&values[0], &values[1], &values[2]) else {
        return None;
    };
    let mut bytes = [0u8; 16];
    bytes[0..4].copy_from_slice(&a.to_be_bytes());
    bytes[4..6].copy_from_slice(&b.to_be_bytes());
    bytes[6..8].copy_from_slice(&c.to_be_bytes());
    for (slot, value) in bytes[8..].iter_mut().zip(&values[3..]) {
        let Value::U8(byte) = value else {
            return None;
        };
        *slot = *byte;
    }
    Some(Iid(bytes))
}

/// Render a metadata type the way an IDL or a header would spell it.
fn type_name(ty: &Type) -> String {
    match ty {
        Type::Void => "void".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Char => "char".to_string(),
        Type::I8 => "i8".to_string(),
        Type::U8 => "u8".to_string(),
        Type::I16 => "i16".to_string(),
        Type::U16 => "u16".to_string(),
        Type::I32 => "i32".to_string(),
        Type::U32 => "u32".to_string(),
        Type::I64 => "i64".to_string(),
        Type::U64 => "u64".to_string(),
        Type::F32 => "f32".to_string(),
        Type::F64 => "f64".to_string(),
        Type::ISize => "isize".to_string(),
        Type::USize => "usize".to_string(),
        Type::String => "HSTRING".to_string(),
        Type::Object => "IInspectable".to_string(),
        Type::ClassName(name) => format!("{}.{}", name.namespace, name.name),
        Type::ValueName(name) => format!("{}.{}", name.namespace, name.name),
        Type::Array(inner) => format!("{}[]", type_name(inner)),
        Type::RefMut(inner) => format!("{}&", type_name(inner)),
        Type::RefConst(inner) => format!("const {}&", type_name(inner)),
        Type::PtrMut(inner, _) => format!("{}*", type_name(inner)),
        Type::PtrConst(inner, _) => format!("const {}*", type_name(inner)),
        Type::ArrayFixed(inner, count) => format!("{}[{count}]", type_name(inner)),
        Type::Generic(name, _) => name.clone(),
    }
}

/// The interfaces named by `StaticAttribute`, which carry the class's static methods.
fn static_interfaces(def: &windows_metadata::reader::TypeDef<'_>) -> Vec<String> {
    def.attributes()
        .filter(|attribute| attribute.name() == "StaticAttribute")
        .filter_map(|attribute| {
            let values = attribute.value();
            match values.first() {
                Some((_, Value::TypeName(name))) => {
                    Some(format!("{}.{}", name.namespace, name.name))
                }
                _ => None,
            }
        })
        .collect()
}

fn implemented(def: &windows_metadata::reader::TypeDef<'_>) -> Vec<String> {
    def.interface_impls()
        .filter_map(|implementation| match implementation.interface(&[]) {
            Type::ClassName(name) => Some(format!("{}.{}", name.namespace, name.name)),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types_render_like_idl() {
        assert_eq!(type_name(&Type::U32), "u32");
        assert_eq!(
            type_name(&Type::RefConst(Box::new(Type::ClassName(
                windows_metadata::TypeName::named("Microsoft.UI.Composition", "IVisual")
            )))),
            "const Microsoft.UI.Composition.IVisual&"
        );
        assert_eq!(type_name(&Type::Array(Box::new(Type::U8))), "u8[]");
    }

    #[test]
    fn an_iid_round_trips_through_its_text_form() {
        let text = "01234567-89ab-cdef-1122-334455667788";
        assert_eq!(Iid::parse(text).unwrap().to_string(), text);
        assert_eq!(
            Iid::parse("{01234567-89AB-CDEF-1122-334455667788}")
                .unwrap()
                .to_string(),
            text
        );
        assert!(Iid::parse("01234567-89ab-cdef-1122").is_none());
        assert!(Iid::parse("zzzz4567-89ab-cdef-1122-334455667788").is_none());
    }

    #[test]
    fn an_iid_prints_in_registry_order() {
        let iid = Iid([
            0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ]);
        assert_eq!(iid.to_string(), "01234567-89ab-cdef-1122-334455667788");
    }
}
