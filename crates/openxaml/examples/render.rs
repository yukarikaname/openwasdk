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
    let source = args
        .next()
        .ok_or("usage: render <page.xbf> <out.ppm> [w h]")?;
    let out = args
        .next()
        .ok_or("usage: render <page.xbf> <out.ppm> [w h]")?;
    let width: u32 = args.next().map_or(Ok(800), |v| v.parse())?;
    let height: u32 = args.next().map_or(Ok(600), |v| v.parse())?;
    // A material blurs what is behind the window; a headless render has no desktop, so one
    // can be supplied as a PPM.
    let backdrop = match args.next().as_deref() {
        Some("--backdrop") => args.next(),
        _ => None,
    };

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
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        dump_layout(&root, &layout, 0);
    }
    let tree = root.compose(&layout);

    // The window in the page's own theme: the Gallery is a light application.
    let background = openxaml::theme::palette::color("SolidBackgroundFillColorBase")
        .or_else(|| openxaml::theme::palette::color("ApplicationPageBackgroundThemeBrush"))
        .map(|c| [c[0], c[1], c[2], 1.0])
        .unwrap_or_else(|| [srgb(0.95), srgb(0.95), srgb(0.95), 1.0]);
    let Some(mut renderer) = Renderer::headless((width, height), background) else {
        return Err("no GPU adapter available".into());
    };
    /*
     * A window that declares a Mica backdrop gets one: the recipe runs over a simulated
     * desktop, because a page render has no wallpaper to sample. The material code is the
     * same `blur_and_tint` the Skia path exposes.
     */
    let mut tree = tree;
    let desktop: Option<Vec<u8>> = backdrop
        .as_deref()
        .and_then(read_ppm)
        .or_else(|| wallpaper(width, height));
    if contains_kind(&root, openxaml::Kind::MicaBackdrop)
        && let Some(desktop) = desktop
    {
        let tint = openxaml::theme::palette::color("SolidBackgroundFillColorBase")
            .unwrap_or([0.95, 0.95, 0.95, 1.0]);
        // WinUI's light Mica lets most of the wallpaper through: a thin tint over a
        // half-strength luminosity layer, not a wash.
        let material = opencomp_core::material::Material {
            tint: [tint[0], tint[1], tint[2], 0.28],
            luminosity: [0.55, 0.55, 0.55],
            blur: 48.0,
            noise: 0.0,
        };
        if let Some(panel) =
            opencomp_core::material::blur_and_tint(&desktop, width, height, width, height, material)
        {
            renderer.upload_rgba(1, width, height, &panel);
            let mut base =
                opencomp_core::Visual::rect(width as f32, height as f32, [1.0, 1.0, 1.0, 1.0]);
            base.image = Some(1);
            base.uv = [0.0, 0.0, 1.0, 1.0];
            base.push(tree);
            tree = base;
        }
    }
    renderer.set_root(tree);
    let pixels = renderer.render_to_pixels();
    if pixels.is_empty() {
        return Err("nothing was rendered".into());
    }

    let mut file = std::io::BufWriter::new(std::fs::File::create(&out)?);
    use std::io::Write;
    write!(file, "P6\n{width} {height}\n255\n")?;
    for pixel in pixels.as_chunks::<4>().0 {
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
        if let Ok(file) = xbf::parse_file(path)
            && let Ok(model) = file.model()
        {
            application_types(&model.roots, &mut named);
        }
    }
    for name in &named {
        controls.insert_unrealized(name);
    }

    controls.set_theme(theme(root));

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
            .is_some_and(|kind| !kind.is_non_visual());
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
            let simple = element
                .type_name
                .rsplit('.')
                .next()
                .unwrap_or(&element.type_name);
            out.push(simple.to_string());
        }
        application_types(&element.children, out);
    }
}

/// The framework's own templates, from the `Controls.pri` beside the application.
///
/// A page says `Button`; what a Button looks like is a `Style` with a `ControlTemplate` in
/// this dictionary, so the loader needs it for anything beyond an empty box.
fn theme(root: &Path) -> openxaml::theme::Theme {
    let pri = root.join("Microsoft.UI.Xaml.Controls.pri");
    let Ok(data) = std::fs::read(&pri) else {
        return openxaml::theme::Theme::empty();
    };
    let mut theme = openxaml::theme::Theme::empty();
    for fragment in xbf::pri::scan(&data) {
        if let Some(file) = xbf::pri::fragment_document(&data, &fragment) {
            theme.merge(openxaml::theme::Theme::from_dictionary(&file));
        }
    }
    eprintln!("theme: {} templates from {}", theme.len(), pri.display());
    theme
}

fn collect(dir: &Path, out: &mut HashMap<String, PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|kind| kind == "xbf")
            && let Some(stem) = path.file_stem().and_then(|stem| stem.to_str())
        {
            out.entry(stem.to_string()).or_insert(path);
        }
    }
}

/// Print what the layout produced for the controls that matter, when tracing.
fn dump_layout(element: &openxaml::Element, layout: &openxaml::Layout, depth: usize) {
    use openxaml::Kind;
    if matches!(
        element.kind(),
        Kind::Button | Kind::TextBlock | Kind::ContentPresenter
    ) {
        eprintln!(
            "{:indent$}{:?} {}x{} at {:.1},{:.1} children {}",
            "",
            element.kind(),
            layout.rect.width,
            layout.rect.height,
            layout.rect.x,
            layout.rect.y,
            layout.children.len(),
            indent = depth * 2
        );
    }
    for (child, child_layout) in element.children().iter().zip(&layout.children) {
        dump_layout(child, child_layout, depth + 1);
    }
}

/// A page that asks for a window backdrop: the material is declared on the window, not drawn.
fn contains_kind(element: &openxaml::Element, kind: openxaml::Kind) -> bool {
    element.kind() == kind
        || element
            .children()
            .iter()
            .any(|child| contains_kind(child, kind))
}

/// A stand-in desktop: a vertical gradient, so a blur has something to soften.
fn wallpaper(width: u32, height: u32) -> Option<Vec<u8>> {
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        let t = y as f32 / height as f32;
        let pixel = [
            (90.0 + 60.0 * t) as u8,
            (120.0 + 40.0 * t) as u8,
            (160.0 - 30.0 * t) as u8,
            255,
        ];
        for x in 0..width {
            let at = ((y * width + x) * 4) as usize;
            pixels[at..at + 4].copy_from_slice(&pixel);
        }
    }
    Some(pixels)
}

/// Read a binary PPM (P6), which is what an outside tool can produce from any picture.
fn read_ppm(path: &str) -> Option<Vec<u8>> {
    let bytes = std::fs::read(path).ok()?;
    let mut fields = Vec::new();
    let mut at = 0;
    while fields.len() < 4 {
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let start = at;
        while at < bytes.len() && !bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        fields.push(std::str::from_utf8(&bytes[start..at]).ok()?.to_string());
    }
    at += 1; // the single whitespace after the header
    if fields[0] != "P6" {
        return None;
    }
    let width: usize = fields[1].parse().ok()?;
    let height: usize = fields[2].parse().ok()?;
    let mut pixels = vec![0u8; width * height * 4];
    for index in 0..width * height {
        let source = at + index * 3;
        if source + 3 > bytes.len() {
            return None;
        }
        pixels[index * 4..index * 4 + 4].copy_from_slice(&[
            bytes[source],
            bytes[source + 1],
            bytes[source + 2],
            255,
        ]);
    }
    Some(pixels)
}
