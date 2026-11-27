/*
 * Stable XBF index tables.
 *
 * Ported from dxaml/xcp/core/Parser/StableXbfIndexes.g.h.  Trusted
 * (stable) ids in a v2 node stream index these ordered name lists
 * instead of the file's own metadata tables.
 */

mod props_0;
mod props_1;
mod props_2;
mod props_3;
mod props_4;
mod props_5;
mod props_6;
mod types_0;
mod types_1;
mod types_2;

static TYPE_CHUNKS: [&[&str]; 3] = [types_0::NAMES, types_1::NAMES, types_2::NAMES];

static PROPERTY_CHUNKS: [&[&str]; 7] = [
    props_0::NAMES,
    props_1::NAMES,
    props_2::NAMES,
    props_3::NAMES,
    props_4::NAMES,
    props_5::NAMES,
    props_6::NAMES,
];

/* Look up a stable type name. */
pub fn type_name(index: u16) -> Option<&'static str> {
    lookup(&TYPE_CHUNKS, index as usize)
}

/* Look up a stable property name. */
pub fn property_name(index: u16) -> Option<&'static str> {
    lookup(&PROPERTY_CHUNKS, index as usize)
}

fn lookup(chunks: &'static [&'static [&'static str]], mut index: usize) -> Option<&'static str> {
    for chunk in chunks {
        if index < chunk.len() {
            return Some(chunk[index]);
        }
        index -= chunk.len();
    }
    None
}
