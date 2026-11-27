//! Compare two metadata files. The Windows App SDK adds methods and interfaces between releases;
//! a stub is pinned to one version, so a report of what moved is the first thing to read before
//! bumping it.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::{Interface, Model};

/// Interfaces added, removed or changed between `old` and `new`.
///
/// Methods are compared by signature, so a parameter that changed type shows up as a removal and
/// an addition on the same interface. IID changes are reported separately because they break
/// binary compatibility even when the shape looks identical.
pub fn diff(old: &Model, new: &Model, namespace: Option<&str>) -> String {
    let keep = |interface: &Interface| namespace.is_none_or(|ns| interface.namespace == ns);
    let before = index(old, keep);
    let after = index(new, keep);

    let mut out = String::new();
    let mut added = 0;
    let mut removed = 0;
    let mut changed = 0;

    for name in after.keys().filter(|name| !before.contains_key(*name)) {
        out.push_str(&format!("+ interface {name}\n"));
        added += 1;
    }
    for name in before.keys().filter(|name| !after.contains_key(*name)) {
        out.push_str(&format!("- interface {name}\n"));
        removed += 1;
    }
    for (name, old_interface) in &before {
        let Some(new_interface) = after.get(name) else {
            continue;
        };
        let mut changes = Vec::new();
        if old_interface.iid != new_interface.iid {
            changes.push(format!(
                "  iid {} -> {}",
                old_interface
                    .iid
                    .map_or_else(|| "-".to_string(), |iid| iid.to_string()),
                new_interface
                    .iid
                    .map_or_else(|| "-".to_string(), |iid| iid.to_string()),
            ));
        }
        let old_methods: BTreeSet<String> = old_interface
            .methods
            .iter()
            .map(|method| method.signature())
            .collect();
        let new_methods: BTreeSet<String> = new_interface
            .methods
            .iter()
            .map(|method| method.signature())
            .collect();
        for signature in new_methods.difference(&old_methods) {
            changes.push(format!("  + {signature}"));
        }
        for signature in old_methods.difference(&new_methods) {
            changes.push(format!("  - {signature}"));
        }
        if !changes.is_empty() {
            out.push_str(&format!("~ interface {name}\n"));
            for change in changes {
                out.push_str(&format!("{change}\n"));
            }
            changed += 1;
        }
    }

    if added == 0 && removed == 0 && changed == 0 {
        return "no interface changes\n".to_string();
    }
    out.push_str(&format!(
        "\n{added} added, {removed} removed, {changed} changed\n"
    ));
    out
}

fn index(model: &Model, keep: impl Fn(&Interface) -> bool) -> BTreeMap<String, &Interface> {
    model
        .interfaces
        .iter()
        .filter(|interface| keep(interface))
        .map(|interface| (interface.full_name(), interface))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Iid, Method};

    fn interface(name: &str, iid: u8, methods: &[&str]) -> Interface {
        Interface {
            namespace: "Test".to_string(),
            name: name.to_string(),
            iid: Some(Iid([iid; 16])),
            methods: methods
                .iter()
                .map(|signature| Method {
                    name: signature.to_string(),
                    returns: "void".to_string(),
                    params: vec![],
                })
                .collect(),
        }
    }

    #[test]
    fn an_unchanged_pair_reports_nothing() {
        let old = Model {
            interfaces: vec![interface("IA", 1, &["One"])],
            ..Model::default()
        };
        let new = Model {
            interfaces: vec![interface("IA", 1, &["One"])],
            ..Model::default()
        };
        assert_eq!(diff(&old, &new, None), "no interface changes\n");
    }

    #[test]
    fn an_iid_change_and_a_new_method_are_reported() {
        let old = Model {
            interfaces: vec![interface("IA", 1, &["One"])],
            ..Model::default()
        };
        let new = Model {
            interfaces: vec![interface("IA", 2, &["One", "Two"])],
            ..Model::default()
        };
        let report = diff(&old, &new, None);
        assert!(report.contains("~ interface Test.IA"));
        assert!(report.contains("iid 01010101-0101-0101-0101-010101010101"));
        assert!(report.contains("  + void Two()"));
        assert!(report.contains("0 added, 0 removed, 1 changed"));
    }
}
