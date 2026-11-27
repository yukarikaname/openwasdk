//! Draw a small tree offscreen and write it as a binary PPM.
//!
//! This is the whole renderer without a window: the same `Visual` tree a
//! `ContainerVisual` produces, the same `render` call a swapchain uses. Run
//! `cargo run -p opencomp-core --example scene -- /tmp/scene.ppm`.

use std::io::Write;

use opencomp_core::{Renderer, Visual};

fn main() -> std::io::Result<()> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "scene.ppm".into());
    let (width, height) = (320, 240);
    let Some(mut renderer) = Renderer::headless((width, height), [0.05, 0.06, 0.08, 1.0]) else {
        eprintln!("no GPU adapter available");
        return Ok(());
    };
    renderer.set_root(scene());
    let pixels = renderer.render_to_pixels();
    if pixels.is_empty() {
        eprintln!("nothing was rendered");
        return Ok(());
    }

    let mut file = std::io::BufWriter::new(std::fs::File::create(&path)?);
    write!(file, "P6\n{width} {height}\n255\n")?;
    for pixel in pixels.as_chunks::<4>().0 {
        file.write_all(&pixel[..3])?;
    }
    println!("wrote {path}");
    Ok(())
}

/// A panel with three rows, one of them translucent and one clipped away.
fn scene() -> Visual {
    let mut panel = Visual::rect(272.0, 192.0, [0.10, 0.11, 0.14, 1.0])
        .at(24.0, 24.0)
        .clip_to(0.0, 0.0, 272.0, 192.0);
    let rows = [
        ([0.29, 0.62, 0.96, 1.0], 0.0),
        ([0.35, 0.85, 0.55, 0.5], 1.0),
        ([0.96, 0.45, 0.35, 1.0], 2.0),
    ];
    for (color, index) in rows {
        panel.push(
            Visual::rect(240.0, 40.0, color)
                .at(16.0, 16.0 + index * 56.0)
                .with_opacity(0.9),
        );
    }
    // Past the panel's right edge, so the clip has to cut it.
    panel.push(Visual::rect(120.0, 24.0, [0.95, 0.85, 0.30, 1.0]).at(220.0, 150.0));
    panel
}
