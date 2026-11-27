//! Text reports. They are the first thing a port needs: what exists, what it is called, and which
//! IIDs a proxy has to answer.

use crate::model::{Class, Interface, Model};

/// `namespace  interfaces  methods  missing IIDs`, one line per namespace plus a total.
pub fn summary(model: &Model) -> String {
    let mut rows: Vec<(String, usize, usize, usize)> = model
        .namespaces()
        .into_iter()
        .map(|namespace| {
            let interfaces: Vec<&Interface> = model
                .interfaces
                .iter()
                .filter(|i| i.namespace == namespace)
                .collect();
            (
                namespace.to_string(),
                interfaces.len(),
                interfaces.iter().map(|i| i.methods.len()).sum(),
                interfaces.iter().filter(|i| i.iid.is_none()).count(),
            )
        })
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let width = rows.iter().map(|r| r.0.len()).max().unwrap_or(9).max(9);
    let mut out = format!(
        "{:<width$}  {:>10}  {:>7}  {:>5}\n",
        "namespace",
        "interfaces",
        "methods",
        "no IID",
        width = width,
    );
    for (namespace, interfaces, methods, missing) in &rows {
        out.push_str(&format!(
            "{:<width$}  {:>10}  {:>7}  {:>5}\n",
            namespace,
            interfaces,
            methods,
            missing,
            width = width,
        ));
    }
    out.push_str(&format!(
        "\n{:>width$} interfaces, {} classes, {} enums, {} structs, {} delegates, {} attributes\n",
        model.interfaces.len(),
        model.classes.len(),
        model.enums,
        model.structs,
        model.delegates,
        model.attributes,
        width = width + 10,
    ));
    out
}

/// One line per interface: `IID  method count  namespace.name`.
pub fn interfaces(model: &Model, namespace: Option<&str>) -> String {
    let selected: Vec<&Interface> = model
        .interfaces
        .iter()
        .filter(|i| namespace.is_none_or(|ns| i.namespace == ns))
        .collect();
    if selected.is_empty() {
        return "no interfaces\n".to_string();
    }
    let width = selected
        .iter()
        .map(|i| i.full_name().len())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for interface in selected {
        let iid = interface.iid.map_or_else(
            || "------------------------------------".to_string(),
            |iid| iid.to_string(),
        );
        out.push_str(&format!(
            "{}  {:>3}  {:<width$}\n",
            iid,
            interface.methods.len(),
            interface.full_name(),
            width = width,
        ));
    }
    out
}

/// The methods of every interface, or of the named one, with their signatures.
pub fn methods(model: &Model, namespace: Option<&str>, interface: Option<&str>) -> String {
    let selected: Vec<&Interface> = model
        .interfaces
        .iter()
        .filter(|i| namespace.is_none_or(|ns| i.namespace == ns))
        .filter(|i| interface.is_none_or(|name| i.name == name || i.full_name() == name))
        .collect();
    if selected.is_empty() {
        return "no interfaces\n".to_string();
    }
    let mut out = String::new();
    for interface in selected {
        out.push_str(&format!(
            "{}{}\n",
            interface.full_name(),
            interface
                .iid
                .map_or_else(String::new, |iid| format!("  {iid}")),
        ));
        for method in &interface.methods {
            out.push_str(&format!("    {}\n", method.signature()));
        }
    }
    out
}

/// One block per runtime class: name, whether it is activatable, and what it implements.
pub fn classes(model: &Model, namespace: Option<&str>) -> String {
    let selected: Vec<&Class> = model
        .classes
        .iter()
        .filter(|c| namespace.is_none_or(|ns| c.namespace == ns))
        .collect();
    if selected.is_empty() {
        return "no classes\n".to_string();
    }
    let mut out = String::new();
    for class in selected {
        out.push_str(&format!(
            "{}{}\n",
            class.full_name(),
            if class.activatable {
                "  [activatable]"
            } else {
                ""
            },
        ));
        for interface in &class.interfaces {
            out.push_str(&format!("    {interface}\n"));
        }
        for interface in &class.statics {
            out.push_str(&format!("    static {interface}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Iid, Method};

    fn model() -> Model {
        Model {
            interfaces: vec![
                Interface {
                    namespace: "Microsoft.UI.Composition".to_string(),
                    name: "ICompositor".to_string(),
                    iid: Some(Iid([0; 16])),
                    methods: (0..4)
                        .map(|n| Method {
                            name: format!("Method{n}"),
                            returns: "void".to_string(),
                            params: vec![],
                        })
                        .collect(),
                },
                Interface {
                    namespace: "Microsoft.UI.Composition".to_string(),
                    name: "IVisual".to_string(),
                    iid: None,
                    methods: (0..12)
                        .map(|n| Method {
                            name: format!("Method{n}"),
                            returns: "void".to_string(),
                            params: vec![],
                        })
                        .collect(),
                },
            ],
            classes: vec![Class {
                namespace: "Microsoft.UI.Composition".to_string(),
                name: "Compositor".to_string(),
                interfaces: vec!["Microsoft.UI.Composition.ICompositor".to_string()],
                statics: vec!["Microsoft.UI.Composition.ICompositorStatics".to_string()],
                activatable: true,
            }],
            ..Model::default()
        }
    }

    #[test]
    fn the_summary_counts_missing_iids() {
        let out = summary(&model());
        assert!(out.contains("Microsoft.UI.Composition"));
        assert!(out.contains(" 2 interfaces"));
        assert!(out.contains("16"));
    }

    #[test]
    fn an_interface_without_an_iid_keeps_the_column() {
        let out = interfaces(&model(), None);
        assert!(out.contains("00000000-0000-0000-0000-000000000000"));
        assert!(out.contains("------------------------------------"));
        assert!(out.contains("Microsoft.UI.Composition.IVisual"));
    }

    #[test]
    fn classes_list_what_they_implement() {
        let out = classes(&model(), Some("Microsoft.UI.Composition"));
        assert!(out.contains("Compositor  [activatable]"));
        assert!(out.contains("Microsoft.UI.Composition.ICompositor"));
        assert!(out.contains("static Microsoft.UI.Composition.ICompositorStatics"));
    }
}
