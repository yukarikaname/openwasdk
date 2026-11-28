//! Name the native stack when the process is about to fail fast.
//!
//! The App SDK reports a fatal error through `RoFailFastWithErrorContextInternal2`, and
//! Wine's stub prints the code but not who raised it. `OPENCOMP_BT` arms a vectored
//! handler that walks the raising thread's stack and prints each frame as the module it
//! belongs to plus its offset, which is enough to say which DLL gave up.

use std::ffi::c_void;
use std::ptr;
use std::sync::Once;

const EXCEPTION_FAIL_FAST: u32 = 0xC000_0602;
const EXCEPTION_ACCESS_VIOLATION: u32 = 0xC000_0005;
const CONTINUE_SEARCH: i32 = 0;
const FROM_ADDRESS: u32 = 4;
const UNCHANGED_REFCOUNT: u32 = 2;

#[repr(C)]
struct ExceptionRecord {
    code: u32,
    flags: u32,
    nested: *mut c_void,
    address: *mut c_void,
    count: u32,
    reserved: u32,
    details: [usize; 15],
}

#[repr(C)]
struct ExceptionPointers {
    record: *mut ExceptionRecord,
    context: *mut c_void,
}

type Handler = unsafe extern "system" fn(*mut ExceptionPointers) -> i32;

#[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
unsafe extern "system" {
    fn AddVectoredExceptionHandler(first: u32, handler: Handler) -> *mut c_void;
    fn GetModuleHandleExA(flags: u32, address: *const u8, module: *mut *mut c_void) -> i32;
    fn GetModuleFileNameA(module: *mut c_void, name: *mut u8, size: u32) -> u32;
}

#[link(name = "ntdll.dll", kind = "raw-dylib", modifiers = "+verbatim")]
unsafe extern "system" {
    fn RtlCaptureStackBackTrace(skip: u32, count: u32, frames: *mut *mut c_void, hash: *mut u32) -> u16;
}

/// Arm the handler once per process when the variable is set.
pub fn install() {
    static ONCE: Once = Once::new();
    if std::env::var_os("OPENCOMP_BT").is_none() {
        return;
    }
    ONCE.call_once(|| unsafe {
        AddVectoredExceptionHandler(1, on_exception);
        eprintln!("opencore: stack handler armed in pid {}", std::process::id());
    });
}

unsafe extern "system" fn on_exception(info: *mut ExceptionPointers) -> i32 {
    let record = unsafe { (*info).record };
    if record.is_null() {
        return CONTINUE_SEARCH;
    }
    let code = unsafe { (*record).code };
    if code != EXCEPTION_FAIL_FAST && code != EXCEPTION_ACCESS_VIOLATION {
        return CONTINUE_SEARCH;
    }
    eprintln!(
        "opencore: exception {code:#x} at {:?} in pid {}",
        unsafe { (*record).address },
        std::process::id()
    );
    let mut frames = [ptr::null_mut(); 24];
    let count = unsafe { RtlCaptureStackBackTrace(0, 24, frames.as_mut_ptr(), ptr::null_mut()) };
    for frame in &frames[..count as usize] {
        unsafe { print_frame(*frame) };
    }
    CONTINUE_SEARCH
}

/// Report one frame as `module+offset`; an address outside every module is printed bare.
unsafe fn print_frame(address: *mut c_void) {
    let mut module: *mut c_void = ptr::null_mut();
    let flags = FROM_ADDRESS | UNCHANGED_REFCOUNT;
    if unsafe { GetModuleHandleExA(flags, address.cast(), &mut module) } == 0 {
        eprintln!("opencore:   {address:?}");
        return;
    }
    let mut name = [0u8; 260];
    let size = unsafe { GetModuleFileNameA(module, name.as_mut_ptr(), 260) };
    let path = String::from_utf8_lossy(&name[..size as usize]);
    let file = path.rsplit('\\').next().unwrap_or(&path);
    let offset = address as usize - module as usize;
    eprintln!("opencore:   {file}+{offset:#x}");
}
