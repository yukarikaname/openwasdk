//! Load a compiled page and draw it, on the machine you are on.
//!
//! This is milestone 3 without a window: `.xbf` in, pixels out, no Windows and no
//! Microsoft DLL. The same three steps a host takes -- load, arrange, compose -- then
//! the renderer from `opencomp-core` writes a PPM.
//!
//! `cargo run -p openxaml --example render -- MainWindow.xbf page.ppm [width height]`

use opencomp_core::{Renderer, srgb};
use openxaml::{Rect, Size};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let source = args.next().ok_or("usage: render <page.xbf> <out.ppm> [w h]")?;
    let out = args.next().ok_or("usage: render <page.xbf> <out.ppm> [w h]")?;
    let width: u32 = args.next().map_or(Ok(800), |v| v.parse())?;
    let height: u32 = args.next().map_or(Ok(600), |v| v.parse())?;

    let xbf = xbf::parse_file(std::path::Path::new(&source))?;
    let model = xbf.model()?;
    let roots = openxaml::load(&model)?;
    let Some(mut root) = roots.into_iter().next() else {
        return Err("the page has no root element".into());
    };

    let size = Size::new(width as f32, height as f32);
    let measured = root.measure(size)?;
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
