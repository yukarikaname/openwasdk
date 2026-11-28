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
        let template = element.setters.iter().any(|setter| {
            setter
                .name
                .as_deref()
                .is_some_and(|name| name.ends_with("Template"))
        });
        if template && let Some(stream) = element.custom_substream {
            match file.stream_values(stream as usize) {
                Ok(values) => {
                    println!("  setter values: {}", values.len());
                    for value in &values {
                        if let Value::Element(one) = &value.value {
                            println!(
                                "    {} -> {} children={} custom={:?} substream={:?}",
                                value.name,
                                one.type_name,
                                one.children.len(),
                                one.custom,
                                one.custom_substream
                            );
                            for child in one.children.iter().take(4) {
                                println!("      child {}", child.type_name);
                            }
                            for property in &one.properties {
                                let kind = match &property.value {
                                    Value::Deferred { substream } => {
                                        format!("deferred({substream})")
                                    }
                                    Value::Element(inner) => {
                                        format!("element({})", inner.type_name)
                                    }
                                    other => format!("{other:?}"),
                                };
                                println!("      {} = {kind}", property.name);
                                if let Value::Deferred { substream } = &property.value {
                                    match file.substream(*substream as usize) {
                                        Ok(body) => {
                                            println!(
                                                "        body substream {substream}: {} roots",
                                                body.roots.len()
                                            );
                                            for root in body.roots.iter().take(2) {
                                                println!("          root {}", root.type_name);
                                            }
                                        }
                                        Err(error) => {
                                            println!("        body {substream} failed: {error:?}")
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(error) => println!("  setter values failed: {error:?}"),
            }
        }
    }
    for child in &element.children {
        visit(file, child, wanted);
    }
}
