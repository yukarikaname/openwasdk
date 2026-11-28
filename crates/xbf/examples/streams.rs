//! Print one stream of an XBF as its own model: the view for a deferred resource such as a
//! storyboard, whose nodes are not reachable from the page's own tree.
//!
//! usage: streams <file.xbf> [stream]
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: streams <file.xbf> [stream]")?;
    let file = xbf::parse_file(Path::new(&path))?;
    match args.next() {
        Some(index) => print!("{}", file.substream(index.parse()?)?.to_text()),
        None => {
            for index in 0..file.streams.len() {
                let Ok(model) = file.substream(index) else {
                    continue;
                };
                println!("== stream {index}, {} roots", model.roots.len());
                for root in &model.roots {
                    println!("  {}", root.type_name);
                }
            }
        }
    }
    Ok(())
}
