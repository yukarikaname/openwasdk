//! `xwinui` turns the Windows App SDK NuGet packages into things a cross-build
//! needs: a compiler layout, an activation manifest and a layout audit.

mod check;
mod layout;
mod manifest;
mod packages;

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
    /// Report which required metadata files and headers a layout has
    Check {
        /// Layout directory written by `xwinui layout`
        #[arg(long)]
        layout: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xwinui: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
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
        Command::Manifest { out, dll, class } => manifest::write(&out, &dll, &class),
        Command::Check { layout } => {
            let report = check::report(&layout)?;
            print!("{}", report.text);
            if report.missing > 0 {
                return Err(format!("{} required file(s) missing", report.missing));
            }
            Ok(())
        }
    }
}
