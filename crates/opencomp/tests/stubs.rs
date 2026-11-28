#![cfg(windows)]

//! The stub table is a document about this crate, so the numbers in it can be checked.
//!
//! `docs/opencomp-stubs.md` is how a reader learns what is real and what is not. It is
//! written by hand and the code moves under it, so the count it states is asserted here:
//! a stale number is worse than no number, because it is believed.

use std::fs;
use std::path::{Path, PathBuf};

fn sources(dir: &Path, into: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).expect("read src");
    for entry in entries {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            sources(&path, into);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            into.push(path);
        }
    }
}

#[test]
fn the_table_counts_the_stubs_the_code_still_has() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    sources(&root.join("src"), &mut files);

    let mut stubs = 0;
    for file in &files {
        let text = fs::read_to_string(file).expect("read source");
        stubs += text.matches("not_implemented()").count();
    }

    let table =
        fs::read_to_string(root.join("../../docs/opencomp-stubs.md")).expect("read the table");
    let line = table
        .lines()
        .find(|line| line.contains("bodies remaining"))
        .expect("the table states a remaining count");
    let stated: usize = line
        .trim_end_matches(" |")
        .rsplit('|')
        .next()
        .and_then(|cell| cell.trim().parse().ok())
        .expect("the remaining count is a number");

    assert_eq!(
        stated, stubs,
        "the table says {stated} stubs, the code has {stubs}"
    );
}
