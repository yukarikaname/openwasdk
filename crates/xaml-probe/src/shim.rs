//! Answers to the process-policy questions the engine asks, where Wine only has stubs.
//!
//! `Microsoft.ui.xaml.dll` asks `AppPolicyGetWindowingModel` whether the process is a
//! UWP host or a classic desktop one. Wine 9 carries that name as a stub that aborts the
//! process when called, and we do not patch Wine: this rewrites the engine's own import
//! slot after it is loaded, which changes nothing outside our process.
//!
//! The table is deliberately small and explicit. An entry either answers a function Wine
//! stubs, or forwards one unchanged while saying when the engine calls it: when a start
//! fails with a bare `E_NOINTERFACE` the interface the engine asked for is the only fact
//! worth having, and nothing on the outside reports it.

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

use windows_core::GUID;
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
    replacement: Replacement,
    /// Where the engine's own pointer is kept, when the replacement forwards to it.
    original: Option<&'static AtomicPtr<c_void>>,
}

enum Replacement {
    /// `AppPolicyGetWindowingModel`: answered without asking Wine.
    WindowingModel,
    /// `RoGetActivationFactory`: traced, then answered by `combase`.
    ActivationFactory,
    /// `GetProcAddress`: one delay-loaded name is answered here instead.
    GetProcAddress,
    /// `NtCreateWaitCompletionPacket`: the App SDK's messaging DLL calls it at session
    /// creation, and Wine has no implementation.
    WaitCompletionPacket,
    /// `NtAssociateWaitCompletionPacket`: the packet is accepted and never fires.
    AssociateWaitCompletionPacket,
    /// `NtCancelWaitCompletionPacket`: nothing was associated, so nothing to cancel.
    CancelWaitCompletionPacket,
}

impl Entry {
    fn pointer(&self) -> *mut c_void {
        match self.replacement {
            Replacement::WindowingModel => {
                windowing_model as unsafe extern "system" fn(*mut c_void, *mut i32) -> i32
                    as *mut c_void
            }
            Replacement::ActivationFactory => {
                activation_factory
                    as unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32
                    as *mut c_void
            }
            Replacement::GetProcAddress => {
                get_proc_address as unsafe extern "system" fn(*mut c_void, *const u8) -> *mut c_void
                    as *mut c_void
            }
            Replacement::WaitCompletionPacket => {
                wait_completion_packet
                    as unsafe extern "system" fn(*mut *mut c_void, u32, *mut c_void) -> i32
                    as *mut c_void
            }
            Replacement::AssociateWaitCompletionPacket => {
                associate_wait_completion_packet
                    as unsafe extern "system" fn(
                        *mut c_void,
                        *mut c_void,
                        *mut c_void,
                        *mut c_void,
                        *mut c_void,
                        i32,
                        u8,
                    ) -> i32 as *mut c_void
            }
            Replacement::CancelWaitCompletionPacket => {
                cancel_wait_completion_packet as unsafe extern "system" fn(*mut c_void, u8) -> i32
                    as *mut c_void
            }
        }
    }
}

/// The engine's own `RoGetActivationFactory`, taken over below.
static RO_GET_ACTIVATION_FACTORY: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

/// Report what the engine asks for, then let `combase` answer.
///
/// A missing interface comes back as `E_NOINTERFACE` with no class and no id attached, so
/// the pair has to be printed here or nowhere.
unsafe extern "system" fn activation_factory(
    class: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    type Original = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
    let traced = std::env::var_os("OPENCOMP_TRACE").is_some();
    if traced && !iid.is_null() {
        let iid = unsafe { *iid };
        let tail: String = iid.data4.iter().map(|byte| format!("{byte:02x}")).collect();
        eprintln!(
            "xaml-probe: asking for {{{:08x}-{:04x}-{:04x}-{tail}}}",
            iid.data1, iid.data2, iid.data3
        );
    }
    let original: Original =
        unsafe { std::mem::transmute(RO_GET_ACTIVATION_FACTORY.load(Ordering::Relaxed)) };
    let result = unsafe { original(class, iid, out) };
    if traced {
        eprintln!("xaml-probe:   answered hr=0x{:08x}", result as u32);
    }
    result
}

/// The engine's own `GetProcAddress`, taken over below.
static REAL_GET_PROC_ADDRESS: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

/// `CoreMsgCreateSession(0, &session)` from the App SDK's own messaging DLL.
static CORE_MSG_CREATE_SESSION: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

/// `CoreUICreate` from our own session object, used when the App SDK's is not asked for.
static CORE_UI_CREATE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

/// Answer the delay-loaded `CoreUICreate` with `function` instead of Wine's stub.
pub fn answer_core_ui(function: *mut c_void) {
    CORE_UI_CREATE.store(function, Ordering::Release);
}

/// Answer it with the App SDK's own session, created by `CoreMsgCreateSession(0, &)`.
///
/// The runtime ships a complete CoreMessaging, server side included, and its session is
/// what the engine expects: `CoreUICreate` and `CoreMsgCreateSession` name the same object
/// from the two generations of the API. Wine cannot run it as shipped -- session creation
/// calls `NtCreateWaitCompletionPacket`, which ntdll only stubs -- so the call is answered
/// in [`wait_completion_packet`] and everything above it is Microsoft's code.
pub fn answer_core_ui_with_messaging(module: HMODULE) -> bool {
    let Some(export) = crate::windows::export(module, "CoreMsgCreateSession") else {
        return false;
    };
    CORE_MSG_CREATE_SESSION.store(export as *mut c_void, Ordering::Release);
    true
}

/// `HRESULT CoreUICreate(void **session)` over `CoreMsgCreateSession(0, session)`.
unsafe extern "system" fn core_ui_create(session: *mut *mut c_void) -> i32 {
    type Create = unsafe extern "system" fn(u32, *mut *mut c_void) -> i32;
    let create: Create =
        unsafe { std::mem::transmute(CORE_MSG_CREATE_SESSION.load(Ordering::Acquire)) };
    unsafe { create(0, session) }
}

/// Answer one delay-loaded name, then behave exactly as `kernel32` does.
unsafe extern "system" fn get_proc_address(module: *mut c_void, name: *const u8) -> *mut c_void {
    if !name.is_null() && unsafe { name_matches(name, "CoreUICreate") } {
        if std::env::var_os("OPENCOMP_TRACE").is_some() {
            eprintln!(
                "xaml-probe: GetProcAddress(CoreUICreate) xp={} own={}",
                !CORE_MSG_CREATE_SESSION.load(Ordering::Acquire).is_null(),
                !CORE_UI_CREATE.load(Ordering::Acquire).is_null()
            );
        }
        let from_messaging = CORE_MSG_CREATE_SESSION.load(Ordering::Acquire);
        if !from_messaging.is_null() {
            return core_ui_create as unsafe extern "system" fn(*mut *mut c_void) -> i32
                as *mut c_void;
        }
        let own = CORE_UI_CREATE.load(Ordering::Acquire);
        if !own.is_null() {
            return own;
        }
    }
    type GetProcAddress = unsafe extern "system" fn(*mut c_void, *const u8) -> *mut c_void;
    let real: GetProcAddress =
        unsafe { std::mem::transmute(REAL_GET_PROC_ADDRESS.load(Ordering::Relaxed)) };
    unsafe { real(module, name) }
}

/// `NtCreateWaitCompletionPacket(handle, access, attributes)`: Wine has no such object.
///
/// The messaging DLL waits for completion packets through the thread pool; an event is
/// the closest waitable object a user-mode stand-in can hand back. Nothing in the session
/// protocol is known to observe the difference, and this is the only call the DLL makes
/// that Wine cannot answer.
unsafe extern "system" fn wait_completion_packet(
    handle: *mut *mut c_void,
    _access: u32,
    _attributes: *mut c_void,
) -> i32 {
    #[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
    unsafe extern "system" {
        fn CreateEventW(
            attributes: *mut c_void,
            manual_reset: i32,
            initial_state: i32,
            name: *const u16,
        ) -> *mut c_void;
    }
    if handle.is_null() {
        return 0xC000_000Du32 as i32; // STATUS_INVALID_PARAMETER
    }
    let event = unsafe { CreateEventW(ptr::null_mut(), 1, 0, ptr::null()) };
    if event.is_null() {
        return 0xC000_0017u32 as i32; // STATUS_NO_MEMORY
    }
    unsafe { *handle = event };
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!("xaml-probe: NtCreateWaitCompletionPacket answered with an event");
    }
    0
}

/// `NtAssociateWaitCompletionPacket(packet, port, target, key, context, status, signaled)`:
/// without a real packet object there is nothing to associate, and the association only
/// decides when a wait already made completes. It is accepted and never fires.
unsafe extern "system" fn associate_wait_completion_packet(
    _packet: *mut c_void,
    _port: *mut c_void,
    _target: *mut c_void,
    _key: *mut c_void,
    _context: *mut c_void,
    _status: i32,
    _signaled: u8,
) -> i32 {
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!("xaml-probe: NtAssociateWaitCompletionPacket accepted");
    }
    0
}

/// `NtCancelWaitCompletionPacket(packet, wake)`: nothing was associated.
unsafe extern "system" fn cancel_wait_completion_packet(_packet: *mut c_void, _wake: u8) -> i32 {
    0
}

/// `GetProcAddress(LoadLibrary(library), name)`, or nothing when either step fails.
pub(crate) fn entry(library: &[u8], name: &[u8]) -> Option<*mut c_void> {
    #[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }
    let module = unsafe { LoadLibraryA(library.as_ptr()) };
    if module.is_null() {
        return None;
    }
    let found = unsafe { GetProcAddress(module, name.as_ptr()) };
    (!found.is_null()).then_some(found)
}

/// Ask our messaging stand-in to deliver one message to the handler the engine registered.
///
/// The engine builds its core from that callback, so a probe that only answers the
/// registration never gets one.
pub fn deliver_core_ui() -> Option<i32> {
    let entry = entry(b"opencoremsg.dll\0", b"CoreUIDeliver\0")?;
    type Deliver = unsafe extern "system" fn() -> i32;
    let deliver: Deliver = unsafe { std::mem::transmute(entry) };
    Some(unsafe { deliver() })
}

/// The imports a DLL needs answered before it can be used at all.
const LATE_ENTRIES: &[Entry] = &[
    Entry {
        library: "ntdll.dll",
        name: "NtCreateWaitCompletionPacket",
        replacement: Replacement::WaitCompletionPacket,
        original: None,
    },
    Entry {
        library: "ntdll.dll",
        name: "NtAssociateWaitCompletionPacket",
        replacement: Replacement::AssociateWaitCompletionPacket,
        original: None,
    },
    Entry {
        library: "ntdll.dll",
        name: "NtCancelWaitCompletionPacket",
        replacement: Replacement::CancelWaitCompletionPacket,
        original: None,
    },
];

/// Install the entries in `entries` on `module`; see [`install`].
fn install_from(module: HMODULE, entries: &[Entry]) -> Vec<(&'static str, &'static str)> {
    let mut installed = Vec::new();
    for entry in entries {
        if unsafe { patch(module, entry) } {
            installed.push((entry.library, entry.name));
        }
    }
    installed
}

/// Answer the imports a DLL calls before it can be used, such as a wait completion packet.
pub fn install_late(module: HMODULE) -> Vec<(&'static str, &'static str)> {
    install_from(module, LATE_ENTRIES)
}

const ENTRIES: &[Entry] = &[
    Entry {
        library: "KERNEL32.dll",
        name: "AppPolicyGetWindowingModel",
        replacement: Replacement::WindowingModel,
        original: None,
    },
    Entry {
        library: "api-ms-win-core-winrt-l1-1-0.dll",
        name: "RoGetActivationFactory",
        replacement: Replacement::ActivationFactory,
        original: Some(&RO_GET_ACTIVATION_FACTORY),
    },
    Entry {
        library: "KERNEL32.dll",
        name: "GetProcAddress",
        replacement: Replacement::GetProcAddress,
        original: Some(&REAL_GET_PROC_ADDRESS),
    },
];

/// Rewrite the import slots `module` resolved from a stubbed library.
///
/// Returns the names that were installed, so a report can say what was needed rather
/// than leaving it implicit. A module whose imports cannot be read yields an empty list:
/// the caller then runs without the patch and reports whatever fails next.
pub fn install(module: HMODULE) -> Vec<(&'static str, &'static str)> {
    install_from(module, ENTRIES)
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
                let previous = unsafe { ptr::read_unaligned(slot) };
                if let Some(original) = entry.original {
                    original.store(previous, Ordering::Relaxed);
                }
                unsafe { ptr::write_unaligned(slot, entry.pointer()) };
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
