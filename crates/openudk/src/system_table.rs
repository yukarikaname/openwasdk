//! The function table Microsoft.Internal.FrameworkUdk.System.dll provides.
//!
//! The lifted UDK loads that OS component, asks it for a versioned table of function
//! pointers (GetFunctionTableId), validates each entry's size and kind, and installs its
//! own stub wherever the table has nothing. One of those stubs is the visual diagnostics
//! port, and XAML fails fast on it, so on a system without the component the whole
//! application ends before it draws anything.
//!
//! What is implemented here is the table the diagnostics path asks for: entry zero, whose
//! slot at 0x150 registers a port. Every other slot answers S_OK without doing anything,
//! which is the same thing the UDK's own stub does for the features this table does not
//! carry, minus the failure.
//!
//! Entry zero also carries the table's size and kind, which the UDK checks before using
//! it: 0x1c8 bytes and kind 5.

use std::ffi::c_void;
use std::ptr;
use std::sync::Once;

/// The size the UDK requires of entry zero, and the kind it expects.
const HOST_TABLE_SIZE: u32 = 0x1c8;
const HOST_TABLE_KIND: u32 = 5;

/// Slots the diagnostics wrappers call.
const SLOT_REGISTER: usize = 0x150 / 8;
const SLOT_UNREGISTER: usize = 0x158 / 8;
const SLOT_THIRD: usize = 0x160 / 8;

const SLOTS: usize = HOST_TABLE_SIZE as usize / 8;

static mut HOST_TABLE: [usize; SLOTS] = [0; SLOTS];
static INIT: Once = Once::new();

fn traced() -> bool {
    std::env::var_os("OPENUDK_TRACE").is_some()
}

/// Every slot the diagnostics code does not use answers S_OK, so a call that reaches it
/// returns instead of faulting.
unsafe extern "system" fn quiet(_a: *mut c_void, _b: *mut c_void, _c: *mut c_void) -> i32 {
    0
}

/// The port registration XAML performs while it starts. Nothing listens for the
/// diagnostics connection here, so the callback is never invoked; the call succeeds,
/// which is all the caller requires.
unsafe extern "system" fn register(
    name: *const u16,
    callback: *mut c_void,
    context: *mut c_void,
) -> i32 {
    if traced() {
        eprintln!("openudk: diagnostics port registered at {name:?} for {callback:?} {context:?}");
    }
    0
}

unsafe extern "system" fn unregister() -> i32 {
    if traced() {
        eprintln!("openudk: diagnostics port unregistered");
    }
    0
}

/// Entry point the UDK looks for. A null return means "this table does not have that id",
/// and the UDK keeps its own stub; only entry zero is answered.
///
/// # Safety
/// size and kind must be writable when non-null.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn GetFunctionTableId(
    id: u32,
    size: *mut u32,
    kind: *mut u32,
) -> *mut c_void {
    if id != 0 || size.is_null() || kind.is_null() {
        return ptr::null_mut();
    }
    INIT.call_once(|| unsafe {
        let quiet_entry = quiet as unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void) -> i32;
        let table = &raw mut HOST_TABLE;
        for slot in 0..SLOTS {
            (*table)[slot] = quiet_entry as usize;
        }
        let register_entry =
            register as unsafe extern "system" fn(*const u16, *mut c_void, *mut c_void) -> i32;
        let unregister_entry = unregister as unsafe extern "system" fn() -> i32;
        (*table)[SLOT_REGISTER] = register_entry as usize;
        (*table)[SLOT_UNREGISTER] = unregister_entry as usize;
        (*table)[SLOT_THIRD] = quiet_entry as usize;
    });
    if traced() {
        eprintln!("openudk: function table zero handed out");
    }
    unsafe {
        *size = HOST_TABLE_SIZE;
        *kind = HOST_TABLE_KIND;
        (&raw mut HOST_TABLE).cast()
    }
}
