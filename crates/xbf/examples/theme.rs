/*
 * Follow the chain a control's template lives in: PRI -> theme dictionary -> Style ->
 * deferred setters -> ControlTemplate. Prints what each step found, so the instantiator
 * has something measured to build on.
 *
 * usage: theme <file.pri|file.xbf> [TypeName]
 */

use xbf::model::{Element, Value};
use xbf::{Xbf, pri};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        println!("usage: theme <file.pri|file.xbf> [TypeName]");
        return;
    };
    let wanted = args.next().unwrap_or_else(|| "Button".to_string());
    let data = match std::fs::read(&path) {
        Ok(data) => data,
        Err(error) => {
            eprintln!("{path}: {error}");
            return;
        }
    };
    let files: Vec<Xbf> = if path.ends_with(".pri") {
        pri::scan(&data)
            .iter()
            .filter_map(|fragment| pri::fragment_document(&data, fragment))
            .collect()
    } else {
        match xbf::parse(&data) {
            Ok(file) => vec![file],
            Err(error) => {
                eprintln!("{path}: {error}");
                return;
            }
        }
    };

    for file in &files {
        let Ok(model) = file.model() else { continue };
        for root in &model.roots {
            visit(file, root, &wanted);
        }
    }
}

fn visit(file: &Xbf, element: &Element, wanted: &str) {
    if element.type_name.ends_with("Style")
        && let Some(target) = element
            .properties
            .iter()
            .find(|property| property.name.ends_with("TargetType"))
            .and_then(|property| match &property.value {
                Value::Type { name, .. } => Some(name.clone()),
                _ => None,
            })
        && target.ends_with(wanted)
    {
        let key = element
            .properties
            .iter()
            .find(|property| property.name.contains("Key"))
            .and_then(|property| match &property.value {
                Value::String(key) => Some(key.clone()),
                _ => None,
            });
        println!("style for {target} key={key:?}");
        for property in &element.properties {
            let kind = match &property.value {
                Value::Deferred { substream } => format!("deferred({substream})"),
                Value::Element(one) => format!("element({})", one.type_name),
                Value::Elements(many) => format!("elements({})", many.len()),
                other => format!("{other:?}"),
            };
            println!("    {} = {kind}", property.name);
        }
        println!("    children: {}", element.children.len());
        println!(
            "    custom: {:?} substream: {:?}",
            element.custom, element.custom_substream
        );
        println!("    setters: {}", element.setters.len());
        for setter in &element.setters {
            println!(
                "    setter flags=0x{:02X} name={:?} value={:?}",
                setter.flags, setter.name, setter.value
            );
        }
        let stream = element
            .custom_substream
            .map(|s| s as usize)
            .or_else(|| deferred_stream(element));
        if let Some(stream) = stream {
            match file.substream(stream) {
                Ok(setters) => {
                    println!(
                        "  setters substream {stream}: {} roots",
                        setters.roots.len()
                    );
                    for setter in &setters.roots {
                        println!(
                            "    setter type={} value={:?}",
                            setter.type_name, setter.properties
                        );
                    }
                }
                Err(error) => println!("  substream {stream} failed: {error:?}"),
            }
        }
    }
    for child in &element.children {
        visit(file, child, wanted);
    }
}

/// The first deferred stream this element carries, wherever it appears.
fn deferred_stream(element: &Element) -> Option<usize> {
    for property in &element.properties {
        if let Value::Deferred { substream } = &property.value {
            return Some(*substream as usize);
        }
    }
    element.children.iter().find_map(deferred_stream)
}
