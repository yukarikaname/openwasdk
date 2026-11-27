//! `xwinui` turns the Windows App SDK NuGet packages into things a cross-build
//! needs: a compiler layout, an activation manifest, a layout audit, and a
//! `dotnet build` whose XAML compiler runs under Wine.

mod activation;
mod appx;
mod build;
mod check;
mod layout;
mod manifest;
mod packages;
mod probe;

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
    /// Download the App SDK packages and write a clang-cl layout
    Layout {
        /// Directory to write the layout into
        #[arg(long)]
        out: PathBuf,
        /// Where the .nupkg files are cached; defaults to ~/.cache/xwinui
        #[arg(long)]
        cache: Option<PathBuf>,
    },
    /// Write an application manifest that activates classes in a DLL
    Manifest {
        /// Manifest file to write
        #[arg(long)]
        out: PathBuf,
        /// DLL that implements the classes
        #[arg(long, default_value = manifest::DEFAULT_DLL)]
        dll: String,
        /// Class to register; repeat for more. Defaults to the OpenWASDK classes
        #[arg(long = "class")]
        class: Vec<String>,
    },
    /// Write the SDK's runtime-class table as an activation manifest
    Activation {
        /// Unpacked SDK directory or an MSIX that holds AppxManifest.xml
        #[arg(long)]
        sdk: PathBuf,
        /// Replace a class or class prefix with a DLL; repeat for more
        #[arg(long = "replace", value_name = "CLASS_OR_PREFIX=DLL")]
        replace: Vec<String>,
        /// Manifest file to write
        #[arg(long)]
        out: PathBuf,
    },
    /// Report which required metadata files and headers a layout has
    Check {
        /// Layout directory written by `xwinui layout`
        #[arg(long)]
        layout: PathBuf,
    },
    /// Build a .NET WinUI 3 project, running its XAML compiler under Wine
    ///
    /// The WinUI 3 targets run `tools/net472/XamlCompiler.exe` as a build step
    /// (`Microsoft.UI.Xaml.Markup.Compiler.interop.targets`, property
    /// `XamlCompilerExePath`). That file is a .NET Framework PE, which no
    /// non-Windows kernel can execute, so `build` writes an executable wrapper
    /// script that runs the real compiler under Wine and passes its path as
    /// `XamlCompilerExePath`. Wine is needed only for that step: nothing here
    /// runs the application.
    Build {
        /// Directory holding the project (or the project file itself)
        #[arg(long)]
        project: PathBuf,
        /// Build configuration
        #[arg(long, default_value = "Debug")]
        configuration: String,
        /// wine executable; defaults to `wine` on PATH
        #[arg(long)]
        wine: Option<PathBuf>,
        /// Where the wrapper is written; defaults to ~/.cache/xwinui
        #[arg(long)]
        cache: Option<PathBuf>,
    },
    /// List the .exe, .dll and .xbf files a build produced, with sizes
    Probe {
        /// Directory holding the project (or the project file itself)
        #[arg(long)]
        project: PathBuf,
        /// Build configuration to look in
        #[arg(long, default_value = "Debug")]
        configuration: String,
    },
}

/// An error plus the exit status it deserves. A missing Wine is not a failed
/// build: it is a step this host cannot perform, hence its own code.
#[derive(Debug)]
struct Failure {
    message: String,
    code: u8,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure { message, code: 1 }
    }
}

impl Failure {
    fn step(message: impl Into<String>, code: u8) -> Self {
        Failure {
            message: message.into(),
            code,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure { message, code }) => {
            eprintln!("xwinui: {message}");
            ExitCode::from(code)
        }
    }
}

/// Split `CLASS_OR_PREFIX=DLL` rules. An empty side is a usage error, not a
/// rule that silently matches every class or names no DLL.
fn parse_replacements(raw: &[String]) -> Result<Vec<(String, String)>, Failure> {
    let mut rules = Vec::new();
    for rule in raw {
        let (pattern, dll) = rule
            .split_once('=')
            .ok_or_else(|| format!("--replace {rule}: expected CLASS_OR_PREFIX=DLL"))?;
        if pattern.is_empty() || dll.is_empty() {
            return Err(format!("--replace {rule}: expected CLASS_OR_PREFIX=DLL").into());
        }
        rules.push((pattern.to_string(), dll.to_string()));
    }
    Ok(rules)
}

fn run(cli: Cli) -> Result<(), Failure> {
    match cli.command {
        Command::Layout { out, cache } => {
            let cache = match cache {
                Some(cache) => cache,
                None => packages::default_cache()?,
            };
            let summary = layout::run(&out, &cache)?;
            println!(
                "{} package(s), {} file(s), {} byte(s)",
                summary.packages, summary.files, summary.bytes
            );
            println!("layout at {}", out.display());
            Ok(())
        }
        Command::Manifest { out, dll, class } => {
            manifest::write(&out, &dll, &class).map_err(Failure::from)
        }
        Command::Activation { sdk, replace, out } => {
            let replacements = parse_replacements(&replace)?;
            let xml = activation::read(&sdk)?;
            let entries = activation::parse(&xml)?;
            let rules: Vec<(&str, &str)> = replacements
                .iter()
                .map(|(pattern, dll)| (pattern.as_str(), dll.as_str()))
                .collect();
            let text = activation::render(&entries, &rules);
            activation::write(&out, &text)?;
            let dlls = text.matches("<file name=\"").count();
            eprintln!("{} classes in {} DLLs", entries.len(), dlls);
            Ok(())
        }
        Command::Check { layout } => {
            let report = check::report(&layout)?;
            print!("{}", report.text);
            if report.missing > 0 {
                return Err(format!("{} required file(s) missing", report.missing).into());
            }
            Ok(())
        }
        Command::Build {
            project,
            configuration,
            wine,
            cache,
        } => {
            let cache = match cache {
                Some(cache) => cache,
                None => packages::default_cache()?,
            };
            build::run(&build::Request {
                project: &project,
                configuration: &configuration,
                wine: wine.as_deref(),
                cache: &cache,
            })
        }
        Command::Probe {
            project,
            configuration,
        } => {
            let report = probe::report(&project, &configuration)?;
            print!("{}", report);
            Ok(())
        }
    }
}
