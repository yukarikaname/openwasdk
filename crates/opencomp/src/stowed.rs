//! Read a stowed exception before the process dies from it.
//!
//! `0xC000027B` carries its payload out of band: `ExceptionInformation[0]` points at a
//! `STOWED_EXCEPTION_INFORMATION` with the failure's HRESULT and the stack the failing
//! frame recorded. Without a debugger that structure is the only place the real cause is
//! written down, so a vectored handler prints it and lets the exception continue.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

const STOWED_EXCEPTION: u32 = 0xC000_027B;
const CONTINUE_SEARCH: i32 = 0;

#[repr(C)]
struct ExceptionRecord {
    code: u32,
    flags: u32,
    record: *mut ExceptionRecord,
    address: *mut c_void,
    number_parameters: u32,
    padding: u32,
    information: [usize; 15],
}

#[repr(C)]
struct ExceptionPointers {
    record: *mut ExceptionRecord,
    context: *mut c_void,
}

static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Install the handler once, from the first activation this DLL answers.
pub(crate) fn install() {
    if INSTALLED.swap(true, Ordering::Relaxed) {
        return;
    }
    #[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
    unsafe extern "system" {
        fn AddVectoredExceptionHandler(first: u32, handler: *const c_void) -> *mut c_void;
        fn GetModuleFileNameW(module: *mut c_void, name: *mut u16, size: u32) -> u32;
        fn RtlPcToFileHeader(pc: *mut c_void, base: *mut *mut c_void) -> *mut c_void;
    }

    unsafe extern "system" fn on_exception(pointers: *mut ExceptionPointers) -> i32 {
        if pointers.is_null() {
            return CONTINUE_SEARCH;
        }
        let record = unsafe { (*pointers).record };
        if record.is_null() || unsafe { (*record).code } != STOWED_EXCEPTION {
            return CONTINUE_SEARCH;
        }
        if std::env::var_os("OPENCOMP_TRACE").is_none() {
            return CONTINUE_SEARCH;
        }
        unsafe { report(record) };
        CONTINUE_SEARCH
    }

    /// `STOWED_EXCEPTION_INFORMATION_V2`: header, HRESULT, form, address, stack words.
    unsafe fn report(record: *mut ExceptionRecord) {
        let stowed = unsafe { (*record).information[0] } as *const u8;
        if stowed.is_null() {
            eprintln!("opencomp: stowed exception with no information");
            return;
        }
        let word = |at: usize| unsafe { std::ptr::read_unaligned(stowed.add(at) as *const u32) };
        let size = word(0);
        let signature = word(4);
        let result = word(8) as i32;
        if size < 40 {
            eprintln!("opencomp: stowed exception too small ({size} bytes)");
            return;
        }
        let address = unsafe { std::ptr::read_unaligned(stowed.add(16) as *const usize) };
        let word_size = word(24);
        let words = word(28);
        let frames = unsafe { std::ptr::read_unaligned(stowed.add(32) as *const usize) };
        eprintln!(
            "opencomp: stowed exception sig={signature:#x} hr={result:#x} address={address:?} stack={frames:?} words={words} word_size={word_size}"
        );
        if frames == 0 || words == 0 || (word_size != 4 && word_size != 8) {
            return;
        }
        for index in 0..words.min(32) as usize {
            let slot = frames + index * word_size as usize;
            let pc = match word_size {
                8 => unsafe { std::ptr::read_unaligned(slot as *const usize) },
                _ => (unsafe { std::ptr::read_unaligned(slot as *const u32) }) as usize,
            };
            if pc == 0 {
                break;
            }
            eprintln!("opencomp:   stowed[{index}] {pc:#x} {}", unsafe {
                module_of(pc)
            });
        }
    }

    unsafe fn module_of(pc: usize) -> String {
        let mut base = std::ptr::null_mut();
        let _ = unsafe { RtlPcToFileHeader(pc as *mut c_void, &mut base) };
        if base.is_null() {
            return String::new();
        }
        let mut name = [0u16; 260];
        let len = unsafe { GetModuleFileNameW(base, name.as_mut_ptr(), name.len() as u32) };
        if len == 0 {
            return format!("+{:#x}", pc - base as usize);
        }
        let name = String::from_utf16_lossy(&name[..len as usize]);
        let short = name.rsplit('\\').next().unwrap_or(&name);
        format!("{short}+{:#x}", pc - base as usize)
    }

    unsafe {
        AddVectoredExceptionHandler(1, on_exception as *const c_void);
    }
}
