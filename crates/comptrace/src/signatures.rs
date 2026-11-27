//! Reading `_Impl` traits back out of a generated bindings file.
//!
//! windows-bindgen writes a same-namespace type bare and a cross-namespace one through a
//! `super::` chain. This module turns those declarations into code that compiles in the
//! caller's module, without a second interpretation of the metadata.

use std::collections::BTreeSet;

/// The text between the braces of one `_Impl` trait.
pub fn trait_body(source: &str, namespace: &str, name: &str) -> Option<String> {
    let wanted = format!("{name}_Impl");
    let bytes = source.as_bytes();
    let mut modules: Vec<(String, usize)> = Vec::new();
    let mut depth = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        let rest = &source[index..];
        if rest.starts_with("//") {
            index += rest.find('\n').unwrap_or(rest.len());
            continue;
        }
        if rest.starts_with("/*") {
            index += rest.find("*/").map_or(rest.len(), |end| end + 2);
            continue;
        }
        if rest.starts_with('"') {
            index = string_end(source, index);
            continue;
        }
        if rest.starts_with("pub mod ") {
            let after = &source[index + 8..];
            let end = identifier_end(after);
            if end > 0
                && let Some(brace) = after[end..].find('{')
            {
                modules.push((after[..end].to_string(), depth + 1));
                depth += 1;
                index += 8 + end + brace + 1;
                continue;
            }
        }
        if rest.starts_with("pub trait ") {
            let after = &source[index + 10..];
            let end = identifier_end(after);
            if after[..end] == wanted && path(&modules) == namespace {
                let open = index + 10 + end + after[end..].find('{')?;
                let close = matching_brace(source, open)?;
                return Some(source[open + 1..close].to_string());
            }
        }
        match bytes[index] {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                while modules.last().is_some_and(|(_, opened)| *opened > depth) {
                    modules.pop();
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn path(modules: &[(String, usize)]) -> String {
    modules
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn identifier_end(text: &str) -> usize {
    text.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(text.len())
}

fn string_end(text: &str, open: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = open + 1;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index += 2;
        } else if bytes[index] == b'"' {
            return index + 1;
        } else {
            index += 1;
        }
    }
    bytes.len()
}

fn matching_brace(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut index = open;
    while index < bytes.len() {
        match bytes[index] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

/// Every `fn ...;` in a trait body, rewritten to compile outside the bindings module.
pub fn methods(
    body: &str,
    namespace: &str,
    types: &BTreeSet<String>,
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(start) = next_fn(body, from) {
        let end = body[start..]
            .find(';')
            .map(|offset| start + offset)
            .ok_or_else(|| format!("{namespace}: unterminated method"))?;
        let line = body[..start].rfind('\n').map_or(0, |position| position + 1);
        let indent = start - line;
        let declaration = dedent(&body[start..end], indent);
        let declaration = rewrite_super(&declaration, namespace)?;
        let declaration = qualify(&declaration, namespace, types);
        out.push(underscore_params(&declaration));
        from = end + 1;
    }
    Ok(out)
}

fn next_fn(text: &str, from: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = from;
    while let Some(found) = text[index..].find("fn ") {
        let position = index + found;
        if position == 0 || !is_ident(bytes[position - 1]) {
            return Some(position);
        }
        index = position + 3;
    }
    None
}

/// Strip the indentation the trait body had, so the method can be indented into its impl.
fn dedent(text: &str, indent: usize) -> String {
    let prefix = " ".repeat(indent);
    let mut lines = text.lines();
    let mut out = lines.next().unwrap_or_default().to_string();
    for line in lines {
        out.push('\n');
        out.push_str(
            line.strip_prefix(&prefix)
                .unwrap_or_else(|| line.trim_start()),
        );
    }
    out
}

/// Replace `super::` chains with absolute paths under `crate::bindings`.
fn rewrite_super(text: &str, namespace: &str) -> Result<String, String> {
    let parts: Vec<&str> = namespace.split('.').collect();
    let mut out = String::new();
    let mut rest = text;
    while let Some(position) = rest.find("super::") {
        out.push_str(&rest[..position]);
        let mut after = &rest[position..];
        let mut count = 0;
        while let Some(stripped) = after.strip_prefix("super::") {
            count += 1;
            after = stripped;
        }
        if count > parts.len() {
            return Err(format!(
                "{namespace}: path `{text}` climbs above the bindings"
            ));
        }
        out.push_str("crate::bindings::");
        for part in &parts[..parts.len() - count] {
            out.push_str(part);
            out.push_str("::");
        }
        rest = after;
    }
    out.push_str(rest);
    Ok(out)
}

/// Prefix a bare same-namespace type with its path under `crate::bindings`.
fn qualify(text: &str, namespace: &str, types: &BTreeSet<String>) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < bytes.len() {
        if is_ident_start(bytes[index]) {
            let start = index;
            index += 1;
            while index < bytes.len() && is_ident(bytes[index]) {
                index += 1;
            }
            let ident = &text[start..index];
            if should_qualify(text, start, index, ident, types) {
                out.push_str("crate::bindings::");
                for part in namespace.split('.') {
                    out.push_str(part);
                    out.push_str("::");
                }
                out.push_str(ident);
            } else {
                out.push_str(ident);
            }
        } else {
            out.push(bytes[index] as char);
            index += 1;
        }
    }
    out
}

fn should_qualify(
    text: &str,
    start: usize,
    end: usize,
    ident: &str,
    types: &BTreeSet<String>,
) -> bool {
    if !types.contains(ident) {
        return false;
    }
    let previous = text[..start].chars().rev().find(|c| !c.is_whitespace());
    let next = text[end..].chars().find(|c| !c.is_whitespace());
    // After `::` this is a path segment, not a bare type.
    if previous == Some(':') && text[..start].trim_end().ends_with("::") {
        return false;
    }
    // Before `:` this is a parameter name. Before `::` this is a path prefix.
    if next == Some(':') {
        return false;
    }
    previous_word(text, start) != Some("fn")
}

/// Prefix every parameter that is not `self` with `_`. A stub ignores all of them, and a
/// skeleton should compile without unused-variable warnings.
fn underscore_params(declaration: &str) -> String {
    let Some(open) = declaration.find('(') else {
        return declaration.to_string();
    };
    let Some(close) = matching_paren(declaration, open) else {
        return declaration.to_string();
    };
    let inner = &declaration[open + 1..close];
    let bytes = inner.as_bytes();
    let mut insertions = Vec::new();
    let mut start = 0;
    let mut depth = 0i32;
    let mut index = 0;
    while index <= bytes.len() {
        let boundary = index == bytes.len() || (bytes[index] == b',' && depth == 0);
        if boundary {
            let segment = &inner[start..index];
            if let Some(colon) = parameter_colon(segment) {
                let raw = segment[..colon].trim();
                let name = raw.strip_prefix("mut ").unwrap_or(raw);
                if !name.is_empty()
                    && !name.starts_with('_')
                    && name != "self"
                    && let Some(offset) = segment.find(name)
                {
                    insertions.push(open + 1 + start + offset);
                }
            }
            start = index + 1;
            if index == bytes.len() {
                break;
            }
        } else {
            match bytes[index] {
                b'<' | b'(' | b'[' => depth += 1,
                b'>' | b')' | b']' => depth -= 1,
                _ => {}
            }
        }
        index += 1;
    }
    let mut out = declaration.to_string();
    for position in insertions.into_iter().rev() {
        out.insert(position, '_');
    }
    out
}

fn matching_paren(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    for (index, byte) in bytes.iter().enumerate().skip(open) {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

/// The `:` that separates a parameter's name from its type, not one half of a `::`.
fn parameter_colon(segment: &str) -> Option<usize> {
    let bytes = segment.as_bytes();
    let mut angle = 0i32;
    for index in 1..bytes.len() {
        match bytes[index] {
            b'<' => angle += 1,
            b'>' => angle -= 1,
            b':' if angle == 0
                && bytes[index - 1] != b':'
                && bytes.get(index + 1) != Some(&b':') =>
            {
                return Some(index);
            }
            _ => {}
        }
    }
    None
}

fn previous_word(text: &str, before: usize) -> Option<&str> {
    let text = text[..before].trim_end();
    if text.is_empty() {
        return None;
    }
    let start = text
        .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .map_or(0, |position| position + 1);
    Some(&text[start..])
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_super_path_is_rebased_under_the_bindings_module() {
        assert_eq!(
            rewrite_super(
                "super::super::Foundation::Point",
                "Microsoft.UI.Composition"
            )
            .unwrap(),
            "crate::bindings::Microsoft::Foundation::Point"
        );
        assert_eq!(
            rewrite_super(
                "super::super::super::Windows::Foundation::Numerics::Quaternion",
                "Microsoft.UI.Composition"
            )
            .unwrap(),
            "crate::bindings::Windows::Foundation::Numerics::Quaternion"
        );
    }

    #[test]
    fn a_bare_type_is_qualified_and_a_parameter_name_is_not() {
        let mut types = BTreeSet::new();
        types.insert("Visual".to_string());
        let text = "fn SetParent(&self, value: &Visual) -> windows_core::Result<()>";
        assert_eq!(
            qualify(text, "Microsoft.UI.Composition", &types),
            "fn SetParent(&self, value: &crate::bindings::Microsoft::UI::Composition::Visual) -> windows_core::Result<()>"
        );
    }

    #[test]
    fn a_trait_body_is_found_inside_its_namespace() {
        let source = concat!(
            "pub mod Microsoft {\n",
            "    pub mod UI {\n",
            "        pub trait IVisual_Impl: windows_core::IUnknownImpl {\n",
            "            fn Opacity(&self) -> windows_core::Result<f32>;\n",
            "        }\n",
            "    }\n",
            "}\n",
            "pub mod Windows {\n",
            "    pub mod UI {\n",
            "        pub trait IVisual_Impl: windows_core::IUnknownImpl {\n",
            "            fn Opacity(&self) -> windows_core::Result<f64>;\n",
            "        }\n",
            "    }\n",
            "}\n",
        );
        let body = trait_body(source, "Microsoft.UI", "IVisual").unwrap();
        assert!(body.contains("f32"));
        let body = trait_body(source, "Windows.UI", "IVisual").unwrap();
        assert!(body.contains("f64"));
    }

    #[test]
    fn every_method_gets_a_stub_body() {
        let body = concat!(
            "\n            fn Children(&self) -> windows_core::Result<VisualCollection>;\n",
            "            fn SetParent(\n",
            "                &self,\n",
            "                value: windows_core::Ref<Visual>,\n",
            "            ) -> windows_core::Result<()>;\n",
        );
        let types: BTreeSet<String> = ["Visual".to_string(), "VisualCollection".to_string()]
            .into_iter()
            .collect();
        let methods = methods(body, "Microsoft.UI.Composition", &types).unwrap();
        assert_eq!(methods.len(), 2);
        assert!(methods[0].starts_with("fn Children("));
        assert!(methods[1].contains("crate::bindings::Microsoft::UI::Composition::Visual"));
        assert!(methods[1].contains("windows_core::Ref"));
        assert!(methods[1].contains("_value:"));
    }
}
