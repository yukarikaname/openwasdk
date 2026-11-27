//! `comptrace` inspects WinRT metadata. It is the first step towards an ABI-compatible proxy: the
//! interface list and the IIDs are what a stub or a tracer has to answer.

mod diff;
mod generate;
mod iids;
mod model;
mod pe;
mod report;
mod strings;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Count interfaces, methods and classes per namespace
    Summary { winmd: PathBuf },
    /// List interfaces with their IID and method count
    Interfaces {
        winmd: PathBuf,
        /// Only this namespace
        #[arg(long)]
        namespace: Option<String>,
    },
    /// Print what a PE image imports, optionally only matching DLLs
    Imports {
        image: PathBuf,
        /// Only DLLs whose name contains this, case insensitive
        #[arg(long)]
        dll: Option<String>,
    },
    /// Print what a PE image exports
    Exports { image: PathBuf },
    /// Print the strings in a PE image that start with a prefix
    Strings {
        image: PathBuf,
        /// Only strings that start with this
        #[arg(long)]
        prefix: String,
        /// Read UTF-16LE strings instead of narrow ones
        #[arg(long)]
        wide: bool,
    },
    /// Scan a PE image for the interface identifiers it loads
    Iids {
        image: PathBuf,
        /// Metadata that names identifiers
        #[arg(long = "metadata")]
        metadata: Vec<PathBuf>,
        /// A file of `guid name` lines, for identifiers no metadata defines
        #[arg(long)]
        names: Option<PathBuf>,
        /// Only the identifiers nothing names
        #[arg(long)]
        unknown: bool,
    },
    /// Print the methods of every interface with their signatures
    Methods {
        winmd: PathBuf,
        /// Only this namespace
        #[arg(long)]
        namespace: Option<String>,
        /// Only this interface, by simple or fully qualified name
        #[arg(long)]
        interface: Option<String>,
    },
    /// Generate Rust bindings and implementation traits from metadata
    Generate {
        /// One or more metadata files or directories
        winmd: Vec<PathBuf>,
        /// Namespaces or type prefixes to include, comma separated
        #[arg(long, value_delimiter = ',')]
        namespace: Vec<String>,
        /// File of namespaces, one per line; added to --namespace
        #[arg(long)]
        namespace_file: Option<PathBuf>,
        /// Output file
        #[arg(short, long)]
        out: PathBuf,
        /// Interfaces to emit implementation traits for, comma separated; all when omitted
        #[arg(long, value_delimiter = ',')]
        implement: Vec<String>,
        /// Omit class wrappers and forwarders, leaving the ABI and the implementation traits
        #[arg(long)]
        minimal: bool,
        /// Client view: class wrappers and methods, no --minimal, and every implementation trait
        #[arg(long)]
        client: bool,
        /// Compare the generated code with --out instead of writing it
        #[arg(long)]
        check: bool,
    },
    /// Print every attribute on one type
    Attributes { winmd: PathBuf, type_name: String },
    /// Map an IID from a trace back to its interface, or say that it is not in the metadata
    Lookup { winmd: PathBuf, iid: String },
    /// Compare two metadata files: added, removed and changed interfaces
    Diff {
        old: PathBuf,
        new: PathBuf,
        /// Only this namespace
        #[arg(long)]
        namespace: Option<String>,
    },
    /// List runtime classes and the interfaces they implement
    Classes {
        winmd: PathBuf,
        /// Only this namespace
        #[arg(long)]
        namespace: Option<String>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("comptrace: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Summary { winmd } => {
            let model = model::load(&winmd)?;
            print!("{}", report::summary(&model));
        }
        Command::Interfaces { winmd, namespace } => {
            let model = model::load(&winmd)?;
            print!("{}", report::interfaces(&model, namespace.as_deref()));
        }
        Command::Imports { image, dll } => {
            let dependencies = pe::imports(&image)?;
            print!("{}", pe::report_imports(&dependencies, dll.as_deref()));
        }
        Command::Exports { image } => {
            let exports = pe::exports(&image)?;
            print!("{}", pe::report_exports(&exports));
        }
        Command::Strings {
            image,
            prefix,
            wide,
        } => {
            let found = strings::image(&image, &prefix, wide)?;
            print!("{}", strings::report(&found));
        }
        Command::Iids {
            image,
            metadata,
            names,
            unknown,
        } => {
            let found = iids::image(&image)?;
            let named = iids::names(&metadata, names.as_deref())?;
            let report = iids::report(&found, &named, unknown);
            // The report is long and usually piped into `head`; a closed pipe is not an error.
            let _ = std::io::Write::write_all(&mut std::io::stdout(), report.as_bytes());
        }
        Command::Methods {
            winmd,
            namespace,
            interface,
        } => {
            let model = model::load(&winmd)?;
            print!(
                "{}",
                report::methods(&model, namespace.as_deref(), interface.as_deref())
            );
        }
        Command::Generate {
            winmd,
            namespace,
            namespace_file,
            out,
            implement,
            minimal,
            client,
            check,
        } => {
            let namespaces = generate::namespaces(&namespace, namespace_file.as_deref())?;
            if namespaces.is_empty() {
                return Err("no namespaces: pass --namespace or --namespace-file".to_string());
            }
            let target = if check {
                generate::scratch()
            } else {
                out.clone()
            };
            if !check && let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            let options = generate::Options {
                inputs: &winmd,
                namespaces: &namespaces,
                implement: &implement,
                out: &target,
                minimal,
                client,
            };
            windows_bindgen::bindgen(generate::args(&options));
            if check {
                let result = generate::compare(&out, &target);
                let _ = std::fs::remove_file(&target);
                result?;
                println!("{} is up to date", out.display());
            } else {
                println!("wrote {}", out.display());
            }
        }
        Command::Attributes { winmd, type_name } => {
            for attribute in model::attributes(&winmd, &type_name)? {
                println!("{attribute}");
            }
        }
        Command::Lookup { winmd, iid } => {
            let model = model::load(&winmd)?;
            let Some(iid) = model::Iid::parse(&iid) else {
                return Err(format!("{iid}: not an IID"));
            };
            match model.interface_by_iid(iid) {
                Some(interface) => println!(
                    "{}  {} method(s)",
                    interface.full_name(),
                    interface.methods.len()
                ),
                None => println!("{iid} is not in the metadata"),
            }
        }
        Command::Diff {
            old,
            new,
            namespace,
        } => {
            let before = model::load(&old)?;
            let after = model::load(&new)?;
            print!("{}", diff::diff(&before, &after, namespace.as_deref()));
        }
        Command::Classes { winmd, namespace } => {
            let model = model::load(&winmd)?;
            print!("{}", report::classes(&model, namespace.as_deref()));
        }
    }
    Ok(())
}
