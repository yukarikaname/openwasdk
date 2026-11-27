/*
 * Dump an XBF file as an indented node tree.
 *
 * The indentation follows scope opener/closer nodes, which is the same
 * pre-order structure the XAML object writer sees.  Type, property and
 * namespace references are resolved through the stable index tables or the
 * file's own metadata, depending on bit 15 of the persisted id.
 */

use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

use xbf::{NodeKind, PersistedNode, Xbf};

fn main() -> ExitCode {
    let mut path = None;
    let mut show_metadata = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--metadata" => show_metadata = true,
            "-h" | "--help" => {
                println!("usage: dump [--metadata] <file.xbf>");
                return ExitCode::SUCCESS;
            }
            other => path = Some(other.to_string()),
        }
    }

    let Some(path) = path else {
        eprintln!("usage: dump [--metadata] <file.xbf>");
        return ExitCode::from(2);
    };

    match xbf::parse_file(Path::new(&path)) {
        Ok(file) => {
            print_header(&path, &file);
            if show_metadata {
                print_metadata(&file);
            }
            print_streams(&file);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn print_header(path: &str, file: &Xbf) {
    println!("file: {path}");
    println!(
        "version: {}.{}",
        file.header.version.major, file.header.version.minor
    );
    println!(
        "metadata: {} bytes, node stream: {} bytes",
        file.header.metadata_size, file.header.node_size
    );
    println!("hash: {}", file.metadata.hash);
    println!(
        "tables: {} strings, {} assemblies, {} type namespaces, {} types, {} properties, {} xml namespaces",
        file.metadata.strings.len(),
        file.metadata.assemblies.len(),
        file.metadata.type_namespaces.len(),
        file.metadata.types.len(),
        file.metadata.properties.len(),
        file.metadata.xml_namespaces.len()
    );
    println!(
        "substreams: {}, nodes: {}",
        file.streams.len(),
        file.total_nodes()
    );
}

fn print_metadata(file: &Xbf) {
    println!("\nmetadata tables");
    for (index, value) in file.metadata.strings.iter().enumerate() {
        println!("  string {index}: {value:?}");
    }
    for (index, entry) in file.metadata.assemblies.iter().enumerate() {
        println!(
            "  assembly {index}: {} ({})",
            entry.name,
            xbf::provider_kind_name(entry.provider_kind)
        );
    }
    for (index, entry) in file.metadata.type_namespaces.iter().enumerate() {
        println!("  type namespace {index}: {}", entry.name);
    }
    for (index, entry) in file.metadata.types.iter().enumerate() {
        println!(
            "  type {index}: {} (flags {:#x}, namespace {})",
            entry.name, entry.flags, entry.namespace
        );
    }
    for (index, entry) in file.metadata.properties.iter().enumerate() {
        println!(
            "  property {index}: {} (flags {:#x}, declaring type {})",
            entry.name, entry.flags, entry.declaring_type
        );
    }
    for (index, value) in file.metadata.xml_namespaces.iter().enumerate() {
        println!("  xml namespace {index}: {value}");
    }
}

fn print_streams(file: &Xbf) {
    for (index, stream) in file.streams.iter().enumerate() {
        let node_bytes = stream.line_offset - stream.node_offset;
        println!(
            "\n--- substream {index}: {node_bytes} node bytes, {} nodes",
            stream.nodes.len()
        );
        let mut depth = 0usize;
        for (number, node) in stream.nodes.iter().enumerate() {
            if node.is_closer() {
                depth = depth.saturating_sub(1);
            }
            let indent = "  ".repeat(depth);
            let details = describe(file, &node.kind);
            if details.is_empty() {
                println!("{indent}{number:>4} @{:<5} {}", node.offset, node.name());
            } else {
                println!(
                    "{indent}{number:>4} @{:<5} {:<38} {}",
                    node.offset,
                    node.name(),
                    details
                );
            }
            if node.is_opener() {
                depth += 1;
            }
        }
    }
}

fn describe(file: &Xbf, kind: &NodeKind) -> String {
    let mut text = String::new();
    match kind {
        NodeKind::PushScope | NodeKind::PopScope => {}
        NodeKind::AddNamespace { namespace, prefix } => {
            let _ = write_namespace(&mut text, file, namespace, prefix);
        }
        NodeKind::PushScopeAddNamespace { namespace, prefix } => {
            let _ = write_namespace(&mut text, file, namespace, prefix);
        }
        NodeKind::PushConstant { value } => {
            let _ = write!(text, "value={value}");
        }
        NodeKind::PushResolvedType { ty } => {
            let _ = write!(text, "type={}", file.type_label(ty));
        }
        NodeKind::PushResolvedProperty { property } => {
            let _ = write!(text, "property={}", file.property_label(property));
        }
        NodeKind::SetValue { property }
        | NodeKind::PushScopeGetValue { property }
        | NodeKind::SetValueFromMarkupExtension { property } => {
            let _ = write!(text, "property={}", file.property_label(property));
        }
        NodeKind::AddToDictionaryWithKey { value }
        | NodeKind::SetConnectionId { value }
        | NodeKind::SetName { value }
        | NodeKind::GetResourcePropertyBag { value }
        | NodeKind::ProvideStaticResourceValue { value }
        | NodeKind::ProvideThemeResourceValue { value } => {
            let _ = write!(text, "value={value}");
        }
        NodeKind::CheckPeerType { type_name } => {
            let _ = write!(text, "type={type_name:?}");
        }
        NodeKind::SetCustomRuntimeData { substream, data } => {
            let _ = write!(
                text,
                "substream={substream} {} ({} names)",
                data.kind_name,
                data.names.len()
            );
            for name in data.names.iter().take(4) {
                let _ = write!(text, " {name:?}");
            }
        }
        NodeKind::SetDeferredProperty {
            property,
            substream,
            static_resources,
            theme_resources,
        } => {
            let _ = write!(
                text,
                "property={} substream={substream} static={} theme={}",
                file.property_label(property),
                static_resources.len(),
                theme_resources.len()
            );
        }
        NodeKind::PushScopeCreateTypeBeginInit { ty } | NodeKind::CreateTypeBeginInit { ty } => {
            let _ = write!(text, "type={}", file.type_label(ty));
        }
        NodeKind::PushScopeCreateTypeWithConstantBeginInit { ty, value }
        | NodeKind::PushScopeCreateTypeWithTypeConvertedConstantBeginInit { ty, value }
        | NodeKind::CreateTypeWithConstantBeginInit { ty, value }
        | NodeKind::CreateTypeWithTypeConvertedConstantBeginInit { ty, value } => {
            let _ = write!(text, "type={} value={value}", file.type_label(ty));
        }
        NodeKind::SetValueConstant { property, value }
        | NodeKind::SetValueTypeConvertedConstant { property, value }
        | NodeKind::SetValueFromStaticResource { property, value }
        | NodeKind::SetValueFromThemeResource { property, value } => {
            let _ = write!(
                text,
                "property={} value={value}",
                file.property_label(property)
            );
        }
        NodeKind::SetValueTypeConvertedResolvedProperty { property, value }
        | NodeKind::SetValueFromTemplateBinding { property, value } => {
            let _ = write!(
                text,
                "property={} value={}",
                file.property_label(property),
                file.property_label(value)
            );
        }
        NodeKind::SetValueTypeConvertedResolvedType { property, ty } => {
            let _ = write!(
                text,
                "property={} type={}",
                file.property_label(property),
                file.type_label(ty)
            );
        }
        NodeKind::BeginConditionalScope { predicate } => {
            let _ = write!(
                text,
                "predicate={} args={:?}",
                file.type_label(&predicate.ty),
                predicate.arguments
            );
        }
        NodeKind::AddToCollection
        | NodeKind::AddToDictionary
        | NodeKind::EndInitPopScope
        | NodeKind::EndOfStream
        | NodeKind::EndConditionalScope
        | NodeKind::EndInitProvideValuePopScope => {}
    }
    text
}

fn write_namespace(
    text: &mut String,
    file: &Xbf,
    namespace: &PersistedNode,
    prefix: &str,
) -> std::fmt::Result {
    if prefix.is_empty() {
        write!(text, "xmlns={:?}", file.namespace_label(namespace))
    } else {
        write!(text, "xmlns:{prefix}={:?}", file.namespace_label(namespace))
    }
}
