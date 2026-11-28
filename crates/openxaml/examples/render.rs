//! Load a compiled page and draw it, on the machine you are on.
//!
//! This is milestone 3 without a window: `.xbf` in, pixels out, no Windows and no
//! Microsoft DLL. The same three steps a host takes -- load, arrange, compose -- then
//! the renderer from `opencomp-core` writes a PPM.
//!
//! Every other `.xbf` beside the page is loaded as the body of the control whose name it
//! carries, which is how a page's `<controls:ControlExample>` gets a tree at all.
//!
//! `cargo run -p openxaml --example render -- MainWindow.xbf page.ppm [width height]`

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use opencomp_core::{Renderer, srgb};
use openxaml::{Controls, Rect, Size};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let source = args.next().ok_or("usage: render <page.xbf> <out.ppm> [w h]")?;
    let out = args.next().ok_or("usage: render <page.xbf> <out.ppm> [w h]")?;
    let width: u32 = args.next().map_or(Ok(800), |v| v.parse())?;
    let height: u32 = args.next().map_or(Ok(600), |v| v.parse())?;

    let path = Path::new(&source);
    let xbf = xbf::parse_file(path)?;
    let model = xbf.model()?;
    let controls = bodies(&app_root(path));
    let roots = openxaml::load_with(&model, &controls)?;
    let Some(mut root) = roots.into_iter().next() else {
        return Err("the page has no root element".into());
    };

    let measured = root.measure(Size::new(width as f32, height as f32))?;
    eprintln!("measured {measured:?}");
    let layout = root.arrange(Rect::new(0.0, 0.0, width as f32, height as f32))?;
    let tree = root.compose(&layout);

    let background = [srgb(0.09), srgb(0.10), srgb(0.12), 1.0];
    let Some(mut renderer) = Renderer::headless((width, height), background) else {
        return Err("no GPU adapter available".into());
    };
    renderer.set_root(tree);
    let pixels = renderer.render_to_pixels();
    if pixels.is_empty() {
        return Err("nothing was rendered".into());
    }

    let mut file = std::io::BufWriter::new(std::fs::File::create(&out)?);
    use std::io::Write;
    write!(file, "P6\n{width} {height}\n255\n")?;
    for pixel in pixels.chunks_exact(4) {
        file.write_all(&pixel[..3])?;
    }
    eprintln!("wrote {out}");
    Ok(())
}

/// The directory the application was published to: the one that holds `App.xbf`.
///
/// A page lives in a subdirectory of it, so the walk goes up from the page.
fn app_root(page: &Path) -> PathBuf {
    let mut dir = page.parent().unwrap_or(Path::new(".")).to_path_buf();
    for _ in 0..8 {
        if dir.join("App.xbf").is_file() {
            return dir;
        }
        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => break,
        }
    }
    page.parent().unwrap_or(Path::new(".")).to_path_buf()
}

/// Every control body under `root`, keyed by the file name without its extension.
///
/// A control body is the `.xbf` of the class the page names, and the class's simple
/// name is what the file is called; a body that does not parse is skipped, because a
/// page must not fail on a control it never uses.
fn bodies(root: &Path) -> Controls {
    let mut files: HashMap<String, PathBuf> = HashMap::new();
    collect(root, &mut files);
    let mut controls = Controls::default();

    // A type from the application's own namespace is realized only when its XAML holds
    // a tree. Everything else -- a template-only control, a toolkit animation set -- is
    // drawn as the box the page asked for, because its content is built in code.
    let mut named: Vec<String> = Vec::new();
    for path in files.values() {
        if let Ok(file) = xbf::parse_file(path) {
            if let Ok(model) = file.model() {
                application_types(&model.roots, &mut named);
            }
        }
    }
    for name in &named {
        controls.insert_unrealized(name);
    }

    for (name, path) in files {
        let Ok(file) = xbf::parse_file(&path) else {
            continue;
        };
        let Ok(model) = file.model() else {
            continue;
        };
        let visual = model
            .roots
            .first()
            .and_then(|root| openxaml::Kind::from_type_name(&root.type_name))
            .is_some_and(|kind| !openxaml::Element::is_non_visual(kind));
        if !visual {
            continue;
        }
        controls.insert(&name, model);
    }
    controls
}

/// Every element type the model names in the application's own namespace.
fn application_types(elements: &[xbf::model::Element], out: &mut Vec<String>) {
    for element in elements {
        if element.type_name.starts_with("using:") {
            let simple = element.type_name.rsplit('.').next().unwrap_or(&element.type_name);
            out.push(simple.to_string());
        }
        application_types(&element.children, out);
    }
}

fn collect(dir: &Path, out: &mut HashMap<String, PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|kind| kind == "xbf") {
            if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                out.entry(stem.to_string()).or_insert(path);
            }
        }
    }
}
