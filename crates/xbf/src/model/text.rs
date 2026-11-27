/*
 * Indented text rendering of the model.
 *
 * An element is one line with its type, namespace and custom data kind.
 * Every property is then printed as "Type.property = value", followed by the
 * element children one level deeper.
 */

use std::fmt::Write as _;

use super::{Element, Model, Property, Value};

pub(super) fn render(model: &Model) -> String {
    let mut out = String::new();
    for root in &model.roots {
        write_element(&mut out, root, 0);
    }
    out
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

fn write_element(out: &mut String, element: &Element, depth: usize) {
    indent(out, depth);
    let _ = write!(out, "{}", element.type_name);
    if !element.namespace.is_empty() {
        let _ = write!(out, " [{}]", element.namespace);
    }
    if let Some(custom) = &element.custom {
        let _ = write!(out, " custom={custom}");
    }
    out.push('\n');

    for property in &element.properties {
        write_property(out, element, property, depth + 1);
    }
    for child in &element.children {
        write_element(out, child, depth + 1);
    }
}

fn write_property(out: &mut String, element: &Element, property: &Property, depth: usize) {
    match &property.value {
        Value::Element(child) => {
            indent(out, depth);
            let _ = writeln!(
                out,
                "{}.{} = {}",
                element.type_name, property.name, child.type_name
            );
            write_element(out, child, depth + 1);
        }
        Value::Elements(children) => {
            indent(out, depth);
            let _ = writeln!(
                out,
                "{}.{} = [{} elements]",
                element.type_name,
                property.name,
                children.len()
            );
            for child in children {
                write_element(out, child, depth + 1);
            }
        }
        value => {
            indent(out, depth);
            let _ = writeln!(
                out,
                "{}.{} = {}",
                element.type_name,
                property.name,
                display(value)
            );
        }
    }
}

fn display(value: &Value) -> String {
    match value {
        Value::None => "none".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::Signed(value) => value.to_string(),
        Value::String(value) => format!("{value:?}"),
        Value::Thickness([left, top, right, bottom]) => {
            format!("thickness({left},{top},{right},{bottom})")
        }
        Value::GridLength { unit, value } => format!("grid({unit},{value})"),
        Value::Color(value) => format!("#{value:08X}"),
        Value::Null => "null".to_string(),
        Value::Enum { type_name, value } => format!("enum({type_name}={value})"),
        Value::Type { name, namespace } => format!("type({name} @ {namespace})"),
        Value::Property { name, namespace } => format!("property({name} @ {namespace})"),
        Value::TemplateBinding { name, namespace } => {
            format!("template-binding({name} @ {namespace})")
        }
        Value::Resource { theme, key } => {
            if *theme {
                format!("theme-resource({key:?})")
            } else {
                format!("static-resource({key:?})")
            }
        }
        Value::Element(element) => format!("element({})", element.type_name),
        Value::Elements(elements) => format!("elements({})", elements.len()),
    }
}
