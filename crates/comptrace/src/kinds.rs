//! Emit the element-kind table from the metadata.
//!
//! A class's family is in the metadata: `Panel`, `ContentControl`, `Control`, `Shape`, or
//! no UIElement at all. That decides what the layout pass does with it, so the table in
//! `openxaml` can be generated instead of typed. What the metadata cannot say is which
//! panels this crate actually lays out -- that is our knowledge, and the three names below
//! are the whole of it.

use std::collections::HashMap;

use crate::model::Model;

/// The panels `openxaml` implements; every other panel is a content host until it does.
const PANELS: &[(&str, &str)] = &[
    ("StackPanel", "Stack"),
    ("Grid", "Grid"),
    ("Canvas", "Canvas"),
];

/// Classes in `namespace` (a prefix) as `kinds!` rows.
pub fn table(model: &Model, namespace: &str) -> String {
    // Every class's base, so "is this a UIElement" is a walk up the chain rather than a
    // guess from the immediate parent.
    let parents: HashMap<String, String> = model
        .classes
        .iter()
        .filter_map(|class| Some((class.full_name(), class.extends.clone()?)))
        .collect();
    let mut rows = Vec::new();
    for class in &model.classes {
        let full = class.full_name();
        if !full.starts_with(namespace) {
            continue;
        }
        // The variant is the simple name; a nested or generic class has no place in the enum.
        let short = class.name.as_str();
        if short.contains('\u{60}') {
            continue;
        }
        let role = role_of(short, &parents, full.as_str());
        rows.push(format!("    ({short}, {short:?}, {full:?}, {role}),"));
    }
    rows.sort();
    rows.join("\n")
}

fn role_of(short: &str, parents: &HashMap<String, String>, full: &str) -> String {
    // A TextBlock paints its string and lays out no children; every other text control has
    // chrome around its content and is a content host.
    if short == "TextBlock" {
        return "Text".to_string();
    }
    if let Some((_, panel)) = PANELS.iter().find(|(name, _)| *name == short) {
        return format!("Panel({panel})");
    }
    // Only a UIElement is laid out; a brush, a style, a trigger or an animation is not.
    if is_a(parents, full, "Microsoft.UI.Xaml.UIElement") {
        return "ContentHost".to_string();
    }
    "NonVisual".to_string()
}

/// Whether `full` derives from `ancestor`, walking the immediate bases.
fn is_a(parents: &HashMap<String, String>, full: &str, ancestor: &str) -> bool {
    let mut current = full;
    for _ in 0..32 {
        let Some(base) = parents.get(current) else {
            return false;
        };
        if base == ancestor {
            return true;
        }
        current = base;
    }
    false
}
