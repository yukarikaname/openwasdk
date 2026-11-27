//! Answers to the process-policy questions the engine asks, where Wine only has stubs.
//!
//! `Microsoft.ui.xaml.dll` asks `AppPolicyGetWindowingModel` whether the process is a
//! UWP host or a classic desktop one. Wine 9 carries that name as a stub that aborts the
//! process when called, and we do not patch Wine: this rewrites the engine's own import
//! slot after it is loaded, which changes nothing outside our process.
//!
//! The table is deliberately small and explicit. Every entry is a function Wine stubs and
//! a Win32 host can answer truthfully; anything else belongs in an issue, not here.

use std::ffi::c_void;
use std::ptr;

use windows_sys::Win32::Foundation::HMODULE;

const IMAGE_IMPORT_DESCRIPTOR_SIZE: usize = 20;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const S_OK: i32 = 0;
/// `AppPolicyWindowingModel_ClassicDesktop`: a Win32 process with a message loop.
const WINDOWING_MODEL_CLASSIC_DESKTOP: i32 = 2;

const PAGE_READWRITE: u32 = 0x04;

#[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
unsafe extern "system" {
    fn VirtualProtect(address: *mut c_void, size: usize, new: u32, old: *mut u32) -> i32;
}

/// The Win32 answer to "which windowing model is this process".
unsafe extern "system" fn windowing_model(_token: *mut c_void, model: *mut i32) -> i32 {
    if model.is_null() {
        return E_POINTER;
    }
    unsafe { *model = WINDOWING_MODEL_CLASSIC_DESKTOP };
    S_OK
}

/// One function to install, and where the engine imports it from.
struct Entry {
    library: &'static str,
    name: &'static str,
    replacement: unsafe extern "system" fn(*mut c_void, *mut i32) -> i32,
}

const ENTRIES: &[Entry] = &[Entry {
    library: "KERNEL32.dll",
    name: "AppPolicyGetWindowingModel",
    replacement: windowing_model,
}];

/// Rewrite the import slots `module` resolved from a stubbed library.
///
/// Returns the names that were installed, so a report can say what was needed rather
/// than leaving it implicit. A module whose imports cannot be read yields an empty list:
/// the caller then runs without the patch and reports whatever fails next.
pub fn install(module: HMODULE) -> Vec<(&'static str, &'static str)> {
    let mut installed = Vec::new();
    for entry in ENTRIES {
        if unsafe { patch(module, entry) } {
            installed.push((entry.library, entry.name));
        }
    }
    installed
}

unsafe fn patch(module: HMODULE, entry: &Entry) -> bool {
    let base = module as *const u8;
    // One sanity check, then the fixed offsets DOS and NT headers have had since 1993.
    if unsafe { base.read_unaligned() } != b'M' || unsafe { base.add(1).read_unaligned() } != b'Z' {
        return false;
    }
    let nt = unsafe { base.add(read4(base, 0x3C) as usize) };
    if unsafe { read4(nt, 0) } != 0x0000_4550 {
        return false;
    }
    // PE32+: optional header starts 24 bytes into the NT headers, the data directories 112.
    let optional = unsafe { nt.add(24) };
    if unsafe { read2(optional, 0) } != 0x020B {
        return false;
    }
    let directories = unsafe { optional.add(112) };
    let imports_rva = unsafe { read4(directories, 8) } as usize;
    if imports_rva == 0 {
        return false;
    }
    let mut descriptor = unsafe { base.add(imports_rva) };
    loop {
        let original_thunk = unsafe { read4(descriptor, 0) } as usize;
        let name_rva = unsafe { read4(descriptor, 12) } as usize;
        let first_thunk = unsafe { read4(descriptor, 16) } as usize;
        if name_rva == 0 && first_thunk == 0 {
            return false;
        }
        if unsafe { library_matches(base, name_rva, entry.library) } {
            return unsafe { install_in_thunks(base, original_thunk, first_thunk, entry) };
        }
        descriptor = unsafe { descriptor.add(IMAGE_IMPORT_DESCRIPTOR_SIZE) };
    }
}

unsafe fn library_matches(base: *const u8, name_rva: usize, wanted: &str) -> bool {
    let name = unsafe { base.add(name_rva) };
    let mut index = 0;
    for byte in wanted.bytes() {
        let read = unsafe { name.add(index).read_unaligned() };
        // Import tables spell the library in the linker's case; compare case-insensitively.
        if !read.eq_ignore_ascii_case(&byte) {
            return false;
        }
        index += 1;
    }
    (unsafe { name.add(index).read_unaligned() }) == 0
}

/// Walk the name table and write the replacement over the matching import slot.
unsafe fn install_in_thunks(
    base: *const u8,
    original_thunk: usize,
    first_thunk: usize,
    entry: &Entry,
) -> bool {
    // A module with no name table has already been bound; there is nothing to match on.
    if original_thunk == 0 || first_thunk == 0 {
        return false;
    }
    let names = unsafe { base.add(original_thunk) };
    let slots = unsafe { base.add(first_thunk) };
    let mut index = 0usize;
    loop {
        let (value, slot) = unsafe {
            (
                read8(names, index * 8),
                slots.add(index * 8) as *mut *mut c_void,
            )
        };
        if value == 0 {
            return false;
        }
        // The top bit means "imported by ordinal", which carries no name.
        if value >> 63 == 0 {
            let hint = unsafe { base.add(value as usize) };
            let name = unsafe { hint.add(2) };
            if unsafe { name_matches(name, entry.name) } {
                let mut old = 0u32;
                let writable = unsafe { VirtualProtect(slot.cast(), 8, PAGE_READWRITE, &mut old) };
                if writable == 0 {
                    return false;
                }
                unsafe { ptr::write_unaligned(slot, entry.replacement as *mut c_void) };
                let mut ignored = 0u32;
                unsafe { VirtualProtect(slot.cast(), 8, old, &mut ignored) };
                return true;
            }
        }
        index += 1;
    }
}

unsafe fn name_matches(name: *const u8, wanted: &str) -> bool {
    let mut index = 0;
    for byte in wanted.bytes() {
        if (unsafe { name.add(index).read_unaligned() }) != byte {
            return false;
        }
        index += 1;
    }
    (unsafe { name.add(index).read_unaligned() }) == 0
}

unsafe fn read2(base: *const u8, offset: usize) -> u16 {
    u16::from_le_bytes(unsafe {
        [
            base.add(offset).read_unaligned(),
            base.add(offset + 1).read_unaligned(),
        ]
    })
}

unsafe fn read4(base: *const u8, offset: usize) -> u32 {
    let mut bytes = [0u8; 4];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = unsafe { base.add(offset + index).read_unaligned() };
    }
    u32::from_le_bytes(bytes)
}

unsafe fn read8(base: *const u8, offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = unsafe { base.add(offset + index).read_unaligned() };
    }
    u64::from_le_bytes(bytes)
}
