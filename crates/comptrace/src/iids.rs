//! Which interface identifiers an image actually loads.
//!
//! A WinRT client QIs with a GUID that sits in `.rdata` and is loaded by a `lea r64, [rip+disp]`
//! in `.text`. Counting those loads and naming them against metadata is how a version bump is
//! audited: what the client asks for is a fact, what it does with the answer is not.
//!
//! This is a byte scan, not a disassembler. It can miss an identifier that is reached some other
//! way (a table entry, a `mov` of an immediate pair) and it can count a 16 byte constant that
//! happens to look like a GUID, so treat the result as a strong hint, not as proof.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use object::{Object, ObjectSection};

use crate::model::{self, Iid};

/// The `lea r64, [rip + disp32]` ModRM bytes: every form whose base is RIP.
const RIP_RELATIVE: [u8; 8] = [0x05, 0x0d, 0x15, 0x1d, 0x25, 0x2d, 0x35, 0x3d];

/// A 16 byte constant is taken for an identifier when the variant bits are `10` and the version
/// nibble is 4 or 5. Both hold for every GUID WinRT generates; random data matches about one time
/// in thirty.
fn is_identifier(bytes: &[u8]) -> bool {
    if bytes.len() < 16 || bytes[8] & 0xc0 != 0x80 || bytes == [0; 16] {
        return false;
    }
    let version = u16::from_le_bytes([bytes[6], bytes[7]]);
    (0x4000..0x6000).contains(&version)
}

/// A GUID as stored by Windows: the first three fields little endian.
fn iid_from_memory(bytes: &[u8]) -> Iid {
    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&u32::from_le_bytes(bytes[0..4].try_into().unwrap()).to_be_bytes());
    out[4..6].copy_from_slice(&u16::from_le_bytes(bytes[4..6].try_into().unwrap()).to_be_bytes());
    out[6..8].copy_from_slice(&u16::from_le_bytes(bytes[6..8].try_into().unwrap()).to_be_bytes());
    out[8..16].copy_from_slice(&bytes[8..16]);
    Iid(out)
}

/// Count the identifier loads in `code`, resolving them against `data`.
fn scan(code: &[u8], code_addr: u64, data: &[u8], data_addr: u64) -> BTreeMap<Iid, usize> {
    let mut found: BTreeMap<Iid, usize> = BTreeMap::new();
    let end = data_addr + data.len() as u64;
    let mut at = 0;
    while at + 7 <= code.len() {
        let rex = code[at];
        if (rex == 0x48 || rex == 0x4c)
            && code[at + 1] == 0x8d
            && RIP_RELATIVE.contains(&code[at + 2])
        {
            let disp = i32::from_le_bytes(code[at + 3..at + 7].try_into().unwrap());
            let target = (code_addr + at as u64)
                .wrapping_add(7)
                .wrapping_add(disp as i64 as u64);
            if target >= data_addr && target + 16 <= end {
                let slot = (target - data_addr) as usize;
                let bytes = &data[slot..slot + 16];
                if is_identifier(bytes) {
                    *found.entry(iid_from_memory(bytes)).or_insert(0) += 1;
                }
            }
        }
        at += 1;
    }
    found
}

/// Every identifier the image loads, with how many sites load it.
pub fn image(path: &Path) -> Result<BTreeMap<Iid, usize>, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let file =
        object::File::parse(&*bytes).map_err(|error| format!("{}: {error}", path.display()))?;
    let section = |name: &str| -> Result<(u64, Vec<u8>), String> {
        let section = file
            .section_by_name(name)
            .ok_or_else(|| format!("{}: no {name} section", path.display()))?;
        let address = section.address();
        let data = section.data().map_err(|error| error.to_string())?.to_vec();
        Ok((address, data))
    };
    let (code_addr, code) = section(".text")?;
    let (data_addr, data) = section(".rdata")?;
    Ok(scan(&code, code_addr, &data, data_addr))
}

/// Names for identifiers, from metadata and from a file of `guid name` lines.
pub fn names(
    metadata: &[std::path::PathBuf],
    extra: Option<&Path>,
) -> Result<BTreeMap<Iid, String>, String> {
    let mut names = BTreeMap::new();
    for path in metadata {
        let model = model::load(path)?;
        for interface in &model.interfaces {
            if let Some(iid) = interface.iid {
                names.insert(iid, interface.full_name());
            }
        }
    }
    if let Some(path) = extra {
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        for line in text.lines() {
            let mut words = line.split_whitespace();
            let (Some(guid), Some(name)) = (words.next(), words.next()) else {
                continue;
            };
            if let Some(iid) = Iid::parse(guid) {
                names.insert(iid, name.to_string());
            }
        }
    }
    Ok(names)
}

/// The report: one line per identifier, most used first.
pub fn report(
    found: &BTreeMap<Iid, usize>,
    names: &BTreeMap<Iid, String>,
    unknown: bool,
) -> String {
    let named = found.keys().filter(|iid| names.contains_key(iid)).count();
    let mut rows: Vec<(&Iid, usize)> = found.iter().map(|(iid, count)| (iid, *count)).collect();
    rows.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.0.cmp(&right.0.0)));
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} identifiers, {named} named, {} unknown",
        found.len(),
        found.len() - named
    );
    for (iid, count) in rows {
        let name = names.get(iid).map_or("", String::as_str);
        if unknown && !name.is_empty() {
            continue;
        }
        let _ = writeln!(out, "{iid} x{count:<4} {name}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A GUID-shaped constant, in the layout the loader sees.
    fn constant() -> [u8; 16] {
        let registry = Iid::parse("af86e2e0-b12d-4c6a-9c5a-d7aa65101e90")
            .unwrap()
            .0;
        let mut memory = [0u8; 16];
        memory[0..4]
            .copy_from_slice(&u32::from_be_bytes(registry[0..4].try_into().unwrap()).to_le_bytes());
        memory[4..6]
            .copy_from_slice(&u16::from_be_bytes(registry[4..6].try_into().unwrap()).to_le_bytes());
        memory[6..8]
            .copy_from_slice(&u16::from_be_bytes(registry[6..8].try_into().unwrap()).to_le_bytes());
        memory[8..16].copy_from_slice(&registry[8..16]);
        memory
    }

    #[test]
    fn a_guid_shaped_constant_is_accepted() {
        assert!(is_identifier(&constant()));
        assert!(!is_identifier(&[0; 16]));
        // Version 1 is a DCE identifier, not a WinRT one.
        let mut old = constant();
        old[7] = 0x10;
        assert!(!is_identifier(&old));
    }

    #[test]
    fn a_lea_into_rdata_finds_the_identifier() {
        let data = [0u8; 8]
            .iter()
            .copied()
            .chain(constant())
            .collect::<Vec<_>>();
        let data_addr = 0x2000u64;
        // lea rdx, [rip + 0x1001] at 0x1000 lands on 0x2008, the constant after eight zero bytes.
        let mut code = vec![0x48, 0x8d, 0x15, 0x01, 0x10, 0, 0];
        code.resize(32, 0x90);
        let found = scan(&code, 0x1000, &data, data_addr);
        assert_eq!(found.len(), 1);
        let (iid, count) = found.iter().next().unwrap();
        assert_eq!(*count, 1);
        assert_eq!(iid.to_string(), "af86e2e0-b12d-4c6a-9c5a-d7aa65101e90");
    }

    #[test]
    fn an_instruction_that_is_not_a_lea_is_ignored() {
        // `mov rdx, [rip + 8]` has the same ModRM but a different opcode.
        let data = constant().to_vec();
        let mut code = vec![0x48, 0x8b, 0x15, 0, 0, 0, 0];
        code.resize(32, 0x90);
        assert!(scan(&code, 0x1000, &data, 0x2000).is_empty());
    }

    #[test]
    fn the_report_names_what_it_can_and_hides_the_rest() {
        let known = Iid::parse("af86e2e0-b12d-4c6a-9c5a-d7aa65101e90").unwrap();
        let other = Iid::parse("00000000-0000-0000-0000-000000000001").unwrap();
        let found = BTreeMap::from([(known, 3), (other, 1)]);
        let names = BTreeMap::from([(known, "IInspectable".to_string())]);
        let all = report(&found, &names, false);
        assert!(all.starts_with("2 identifiers, 1 named, 1 unknown"));
        assert!(all.contains("IInspectable"));
        let unknown = report(&found, &names, true);
        assert!(!unknown.contains("IInspectable"));
        assert!(unknown.contains("00000000-0000-0000-0000-000000000001"));
    }
}
