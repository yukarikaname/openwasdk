//! The `scaffold` subcommand: turn a runtime class into the Rust skeleton that implements it.
//!
//! The interface list comes from the metadata, the method signatures come from the `_Impl`
//! traits windows-bindgen writes. Generating those traits into a scratch file and reading them
//! back is deliberate: the signatures have to be the ones `generate` produces, not a second
//! interpretation of the metadata.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use crate::generate;
use crate::model::{self, Class, Model};
use crate::signatures;

/// A `scaffold` run, already resolved from the command line.
pub struct Options<'a> {
    pub inputs: &'a [PathBuf],
    pub class: &'a str,
}

struct Interface {
    name: String,
    namespace: String,
    methods: Vec<String>,
}

/// Render the skeleton for one class.
pub fn render(options: &Options<'_>) -> Result<String, String> {
    let model = model::load_all(options.inputs)?;
    let class = model
        .class(options.class)
        .cloned()
        .ok_or_else(|| format!("{}: not in the metadata", options.class))?;
    let (implemented, generic) = implemented(&model, &class);
    let defined: BTreeSet<String> = model
        .types
        .iter()
        .map(|entry| format!("{}.{}", entry.namespace, entry.name))
        .collect();
    let missing: Vec<&String> = implemented
        .iter()
        .filter(|name| !defined.contains(*name))
        .collect();
    if !missing.is_empty() {
        let names: Vec<&str> = missing.iter().map(|name| name.as_str()).collect();
        return Err(format!(
            "metadata does not define {}; pass the file that declares it",
            names.join(", ")
        ));
    }
    if implemented.is_empty() {
        return Err(format!(
            "{}: implements no interface the scaffold can name",
            class.full_name()
        ));
    }

    let source = traits(options.inputs, &implemented)?;
    let types = namespace_types(&model);
    let empty = BTreeSet::new();
    let mut interfaces = Vec::new();
    let mut names = BTreeSet::new();
    for full_name in &implemented {
        let (namespace, name) = split(full_name);
        if !names.insert(name.to_string()) {
            return Err(format!(
                "{full_name}: another interface has the same name; \
                 the implement attribute cannot import both"
            ));
        }
        let body = signatures::trait_body(&source, namespace, name)
            .ok_or_else(|| format!("{full_name}: no _Impl trait in the generated bindings"))?;
        let methods =
            signatures::methods(&body, namespace, types.get(namespace).unwrap_or(&empty))?;
        interfaces.push(Interface {
            name: name.to_string(),
            namespace: namespace.to_string(),
            methods,
        });
    }

    Ok(render_scaffold(options, &class, &generic, &interfaces))
}

/// Every interface the class implements, including the ones it inherits from a base class.
/// Generic interfaces are returned separately: their instantiation is not in the class table.
fn implemented(model: &Model, class: &Class) -> (Vec<String>, Vec<String>) {
    let mut names = BTreeSet::new();
    let mut generic = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut current = Some(class.clone());
    while let Some(class) = current {
        if !visited.insert(class.full_name()) {
            break;
        }
        for interface in &class.interfaces {
            if interface.contains('`') {
                generic.insert(interface.clone());
            } else {
                names.insert(interface.clone());
            }
        }
        current = class
            .extends
            .as_deref()
            .and_then(|name| model.class(name).cloned());
    }
    (names.into_iter().collect(), generic.into_iter().collect())
}

fn split(full_name: &str) -> (&str, &str) {
    full_name.rsplit_once('.').unwrap_or(("", full_name))
}

/// The namespace's type names. windows-bindgen writes a same-namespace type without a path,
/// so a signature that needs `crate::bindings::` has to be told which identifiers are types.
fn namespace_types(model: &Model) -> BTreeMap<String, BTreeSet<String>> {
    let mut types: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for entry in &model.types {
        types
            .entry(entry.namespace.clone())
            .or_default()
            .insert(entry.name.clone());
    }
    types
}

/// Generate a minimal bindings file for exactly these interfaces and return its text.
fn traits(inputs: &[PathBuf], interfaces: &[String]) -> Result<String, String> {
    let out = generate::scratch();
    let mut args = Vec::new();
    for input in inputs {
        args.push("--in".to_string());
        args.push(input.to_string_lossy().into_owned());
    }
    args.push("--out".to_string());
    args.push(out.to_string_lossy().into_owned());
    args.push("--filter".to_string());
    args.extend(interfaces.iter().cloned());
    args.push("--minimal".to_string());
    // No names after `--implement` means every selected interface.
    args.push("--implement".to_string());
    windows_bindgen::bindgen(args);
    let text =
        std::fs::read_to_string(&out).map_err(|error| format!("{}: {error}", out.display()))?;
    let _ = std::fs::remove_file(&out);
    Ok(text)
}

fn render_scaffold(
    options: &Options<'_>,
    class: &Class,
    generic: &[String],
    interfaces: &[Interface],
) -> String {
    let mut out = String::new();
    let inputs = input_names(options.inputs);
    let _ = writeln!(
        out,
        "// Generated by `comptrace scaffold` from {inputs} for {}.",
        class.full_name()
    );
    let _ = writeln!(out, "//");
    let _ = writeln!(
        out,
        "// The method signatures are the `_Impl` traits from the bindings `comptrace generate`"
    );
    let _ = writeln!(
        out,
        "// produces for the same filter. `not_implemented` is local so this file stands alone;"
    );
    let _ = writeln!(out, "// replace it with the module's helper when pasting.");
    for name in generic {
        let _ = writeln!(
            out,
            "//\n// Not emitted: {name} is generic; the class table does not carry its arguments."
        );
    }
    let _ = writeln!(out);

    let methods = interfaces.iter().map(|i| i.methods.len()).sum::<usize>();
    if methods > 0 {
        let _ = writeln!(out, "fn not_implemented<T>() -> windows_core::Result<T> {{");
        let _ = writeln!(
            out,
            "    Err(windows_core::Error::from_hresult(windows_core::HRESULT("
        );
        let _ = writeln!(out, "        0x8000_4001u32 as i32,");
        let _ = writeln!(out, "    )))");
        let _ = writeln!(out, "}}");
        let _ = writeln!(out);
    }

    for interface in interfaces {
        let _ = writeln!(
            out,
            "use crate::bindings::{}::{};",
            module_path(&interface.namespace),
            interface.name
        );
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "#[windows_core::implement(");
    for interface in interfaces {
        let _ = writeln!(out, "    {},", interface.name);
    }
    let _ = writeln!(out, ")]");
    let _ = writeln!(out, "pub struct {} {{}}", class.name);
    let _ = writeln!(out);

    for interface in interfaces {
        let _ = writeln!(
            out,
            "impl crate::bindings::{}::{}_Impl for {}_Impl {{",
            module_path(&interface.namespace),
            interface.name,
            class.name
        );
        for (index, declaration) in interface.methods.iter().enumerate() {
            for line in declaration.lines() {
                let _ = writeln!(out, "    {line}");
            }
            // The last line is the signature without its semicolon; open the body there.
            if let Some(position) = out.rfind('\n') {
                out.insert_str(position, " {");
            }
            let _ = writeln!(out, "        not_implemented()");
            let _ = writeln!(out, "    }}");
            if index + 1 < interface.methods.len() {
                let _ = writeln!(out);
            }
        }
        let _ = writeln!(out, "}}");
        let _ = writeln!(out);
    }
    out
}

fn module_path(namespace: &str) -> String {
    namespace.replace('.', "::")
}

fn input_names(inputs: &[PathBuf]) -> String {
    let mut names: Vec<String> = inputs
        .iter()
        .map(|path| {
            path.file_name().map_or_else(
                || path.display().to_string(),
                |name| name.to_string_lossy().into_owned(),
            )
        })
        .collect();
    names.sort();
    names.dedup();
    names.join(", ")
}
