/*
 * Minimal JSON rendering of the model.
 *
 * The crate has no serialization dependency on purpose; the object shapes are
 * fixed and small enough to write by hand.  The output is one line of valid
 * JSON so it can be piped into any other tool.
 */

use std::fmt::Write as _;

use super::{Element, Model, Property, Value};

pub(super) fn render(model: &Model) -> String {
    let mut out = String::new();
    out.push_str("{\"roots\":[");
    for (index, root) in model.roots.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        write_element(&mut out, root);
    }
    out.push_str("]}\n");
    out
}

fn write_element(out: &mut String, element: &Element) {
    out.push_str("{\"type\":");
    write_string(out, &element.type_name);
    out.push_str(",\"namespace\":");
    write_string(out, &element.namespace);
    out.push_str(",\"custom\":");
    match &element.custom {
        Some(custom) => write_string(out, custom),
        None => out.push_str("null"),
    }
    out.push_str(",\"properties\":[");
    for (index, property) in element.properties.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        write_property(out, property);
    }
    out.push_str("],\"children\":[");
    for (index, child) in element.children.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        write_element(out, child);
    }
    out.push_str("]}");
}

fn write_property(out: &mut String, property: &Property) {
    out.push_str("{\"name\":");
    write_string(out, &property.name);
    out.push_str(",\"namespace\":");
    write_string(out, &property.namespace);
    out.push_str(",\"value\":");
    write_value(out, &property.value);
    out.push('}');
}

fn write_value(out: &mut String, value: &Value) {
    match value {
        Value::None => out.push_str("{\"kind\":\"none\"}"),
        Value::Null => out.push_str("{\"kind\":\"null\"}"),
        Value::Bool(value) => {
            let _ = write!(out, "{{\"kind\":\"bool\",\"value\":{value}}}");
        }
        Value::Float(value) => {
            let _ = write!(out, "{{\"kind\":\"float\",\"value\":{}}}", number(*value));
        }
        Value::Signed(value) => {
            let _ = write!(out, "{{\"kind\":\"signed\",\"value\":{value}}}");
        }
        Value::String(value) => {
            out.push_str("{\"kind\":\"string\",\"value\":");
            write_string(out, value);
            out.push('}');
        }
        Value::Thickness([left, top, right, bottom]) => {
            let _ = write!(
                out,
                "{{\"kind\":\"thickness\",\"value\":[{},{},{},{}]}}",
                number(*left),
                number(*top),
                number(*right),
                number(*bottom)
            );
        }
        Value::GridLength { unit, value } => {
            let _ = write!(
                out,
                "{{\"kind\":\"grid-length\",\"unit\":{unit},\"value\":{}}}",
                number(*value)
            );
        }
        Value::Color(value) => {
            let _ = write!(out, "{{\"kind\":\"color\",\"value\":\"#{value:08X}\"}}");
        }
        Value::Enum { type_name, value } => {
            out.push_str("{\"kind\":\"enum\",\"type\":");
            write_string(out, type_name);
            let _ = write!(out, ",\"value\":{value}}}");
        }
        Value::Type { name, namespace } => {
            out.push_str("{\"kind\":\"type\",\"type\":");
            write_string(out, name);
            out.push_str(",\"namespace\":");
            write_string(out, namespace);
            out.push('}');
        }
        Value::Property { name, namespace } => {
            out.push_str("{\"kind\":\"property\",\"property\":");
            write_string(out, name);
            out.push_str(",\"namespace\":");
            write_string(out, namespace);
            out.push('}');
        }
        Value::TemplateBinding { name, namespace } => {
            out.push_str("{\"kind\":\"template-binding\",\"property\":");
            write_string(out, name);
            out.push_str(",\"namespace\":");
            write_string(out, namespace);
            out.push('}');
        }
        Value::Resource { theme, key } => {
            let kind = if *theme {
                "theme-resource"
            } else {
                "static-resource"
            };
            out.push_str("{\"kind\":\"");
            out.push_str(kind);
            out.push_str("\",\"key\":");
            write_string(out, key);
            out.push('}');
        }
        Value::Deferred { substream } => {
            out.push_str("{\"kind\":\"deferred\",\"substream\":");
            out.push_str(&substream.to_string());
            out.push('}');
        }
        Value::Extension { name } => {
            out.push_str("{\"kind\":\"extension\",\"name\":");
            write_string(out, name);
            out.push('}');
        }
        Value::Element(child) => {
            out.push_str("{\"kind\":\"element\",\"value\":");
            write_element(out, child);
            out.push('}');
        }
        Value::Elements(children) => {
            out.push_str("{\"kind\":\"elements\",\"value\":[");
            for (index, child) in children.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_element(out, child);
            }
            out.push_str("]}");
        }
    }
}

fn number(value: f32) -> String {
    if value.is_finite() {
        value.to_string()
    } else {
        "null".to_string()
    }
}

fn write_string(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            character if (character as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", character as u32);
            }
            character => out.push(character),
        }
    }
    out.push('"');
}
