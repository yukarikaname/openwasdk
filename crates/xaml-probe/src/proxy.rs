//! A logging wrapper around the system CoreUI session, for measurement on Windows.
//!
//! The engine's input core is a private conversation: on Windows the system answers
//! `CoreUICreate`, and nothing documents what the object it returns is expected to do.
//! Under Wine the same call has to be answered from this side, and the only honest way to
//! learn the answers is to watch the engine ask for them where they exist.
//!
//! This proxy forwards every call to the real object and prints the slot, the arguments and
//! the HRESULT. It answers exactly what the real object answers, because the interface it
//! answers is forwarded first, so the engine's own flow is unchanged by being watched.
//!
//! Two slots are known to hand back objects -- 3 the message registrar, 8 an item -- and
//! those are wrapped as well, so the calls on them are in the trace too.

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

use windows_core::GUID;

const S_OK: i32 = 0;
const E_POINTER: i32 = 0x8000_4003u32 as i32;

/// An entry of an `IUnknown`-based vtable.
type Member = *const c_void;

/// The real `CoreUICreate`, from the system `coremessaging.dll`.
static REAL_CREATE: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

/// Answer `CoreUICreate` with a proxy over the system's own session.
pub(crate) fn install_and_answer() -> bool {
    let Some(create) = crate::shim::entry(b"coremessaging.dll\0", b"CoreUICreate\0") else {
        return false;
    };
    REAL_CREATE.store(create, Ordering::Release);
    crate::shim::answer_core_ui(core_ui_create as *mut c_void);
    true
}

/// `HRESULT CoreUICreate(void **session)`: create the real session, then wrap it.
unsafe extern "system" fn core_ui_create(session: *mut *mut c_void) -> i32 {
    if session.is_null() {
        return E_POINTER;
    }
    type Create = unsafe extern "system" fn(*mut *mut c_void) -> i32;
    let create: Create = unsafe { std::mem::transmute(REAL_CREATE.load(Ordering::Acquire)) };
    let mut real: *mut c_void = ptr::null_mut();
    let hr = unsafe { create(&mut real) };
    if hr < 0 || real.is_null() {
        unsafe { *session = real };
        return hr;
    }
    eprintln!("proxy: CoreUICreate returned {real:?}");
    unsafe { *session = proxy_of(real) };
    S_OK
}

/// One object, one real interface behind it, and a reference count.
#[repr(C)]
struct Proxy {
    vtbl: *const Vtbl,
    refs: AtomicU32,
    real: *mut c_void,
}

#[repr(C)]
struct Vtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    /// The real object's own methods, from slot 3 on.
    methods: [Member; 21],
}

// Safety: the table is read-only and holds function addresses.
unsafe impl Sync for Vtbl {}

static PROXY_VTBL: Vtbl = Vtbl {
    query_interface,
    add_ref,
    release,
    methods: [
        registrar as Member, // 3
        slot_4 as Member,
        slot_5 as Member,
        slot_6 as Member,
        slot_7 as Member,
        item as Member, // 8
        slot_9 as Member,
        slot_10 as Member,
        slot_11 as Member,
        slot_12 as Member,
        slot_13 as Member,
        slot_14 as Member,
        slot_15 as Member,
        slot_16 as Member,
        slot_17 as Member,
        slot_18 as Member,
        slot_19 as Member,
        slot_20 as Member,
        slot_21 as Member,
        slot_22 as Member,
        slot_23 as Member,
    ],
};

fn proxy_of(real: *mut c_void) -> *mut c_void {
    Box::into_raw(Box::new(Proxy {
        vtbl: &PROXY_VTBL,
        refs: AtomicU32::new(1),
        real,
    }))
    .cast()
}

/// Seven arguments cover every method of the session and of the objects it hands out.
type Method = unsafe extern "system" fn(
    *mut c_void,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
) -> isize;

/// Call the real object's slot with the same arguments and print what it answered.
unsafe fn forward(this: *mut c_void, slot: usize, args: [usize; 7]) -> isize {
    let proxy = unsafe { &*(this as *const Proxy) };
    let vtable = unsafe { *(proxy.real as *const *const Member) };
    let method: Method = unsafe { std::mem::transmute(*vtable.add(slot)) };
    let hr = unsafe {
        method(
            proxy.real, args[0], args[1], args[2], args[3], args[4], args[5], args[6],
        )
    };
    eprintln!(
        "proxy: slot {slot} ({:x} {:x} {:x} {:x}) hr=0x{:08x}",
        args[0], args[1], args[2], args[3], hr as u32
    );
    hr
}

/// Slot 3: `GetMessageRegistrar(out)`. The registrar is wrapped so its calls are traced.
unsafe extern "system" fn registrar(
    this: *mut c_void,
    out: usize,
    _b: usize,
    _c: usize,
    _d: usize,
    _e: usize,
    _f: usize,
    _g: usize,
) -> isize {
    let hr = unsafe { forward(this, 3, [out, 0, 0, 0, 0, 0, 0]) };
    unsafe { wrap_out(out, hr) };
    hr
}

/// Slot 8: `(argument, out)`. With a null argument the out value is an item.
unsafe extern "system" fn item(
    this: *mut c_void,
    argument: usize,
    out: usize,
    _c: usize,
    _d: usize,
    _e: usize,
    _f: usize,
    _g: usize,
) -> isize {
    let hr = unsafe { forward(this, 8, [argument, out, 0, 0, 0, 0, 0]) };
    unsafe { wrap_out(out, hr) };
    hr
}

/// Replace the interface the real method wrote with a proxy over it.
unsafe fn wrap_out(out: usize, hr: isize) {
    if hr < 0 || out == 0 {
        return;
    }
    let pointer = out as *mut *mut c_void;
    let real = unsafe { *pointer };
    if real.is_null() {
        return;
    }
    unsafe { *pointer = proxy_of(real) };
}

/// Every other slot: forwarded and printed, nothing else.
macro_rules! plain {
    ($($slot:expr => $name:ident),* $(,)?) => {
        $(
            unsafe extern "system" fn $name(
                this: *mut c_void,
                a: usize,
                b: usize,
                c: usize,
                d: usize,
                e: usize,
                f: usize,
                g: usize,
            ) -> isize {
                unsafe { forward(this, $slot, [a, b, c, d, e, f, g]) }
            }
        )*
    };
}

plain! {
    4 => slot_4,
    5 => slot_5,
    6 => slot_6,
    7 => slot_7,
    9 => slot_9,
    10 => slot_10,
    11 => slot_11,
    12 => slot_12,
    13 => slot_13,
    14 => slot_14,
    15 => slot_15,
    16 => slot_16,
    17 => slot_17,
    18 => slot_18,
    19 => slot_19,
    20 => slot_20,
    21 => slot_21,
    22 => slot_22,
    23 => slot_23,
}

unsafe extern "system" fn query_interface(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if iid.is_null() || out.is_null() {
        return E_POINTER;
    }
    // Ask the real object first: the proxy answers what it answers.
    let proxy = unsafe { &*(this as *const Proxy) };
    let vtable = unsafe { *(proxy.real as *const *const Member) };
    type Query = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
    let query: Query = unsafe { std::mem::transmute(*vtable) };
    let hr = unsafe { query(proxy.real, iid, out) };
    eprintln!("proxy: query {:?} hr=0x{:08x}", unsafe { *iid }, hr as u32);
    if hr < 0 {
        return hr;
    }
    // The real reference the query produced is not needed: the proxy holds its own.
    unsafe { release_real(*out) };
    unsafe { *out = ptr::null_mut() };
    unsafe { add_ref(this) };
    unsafe { *out = this };
    S_OK
}

/// Drop one reference on an interface this crate did not write.
unsafe fn release_real(object: *mut c_void) {
    if object.is_null() {
        return;
    }
    let vtable = unsafe { *(object as *const *const Member) };
    let release: unsafe extern "system" fn(*mut c_void) -> u32 =
        unsafe { std::mem::transmute(*vtable.add(2)) };
    unsafe { release(object) };
}

unsafe extern "system" fn add_ref(this: *mut c_void) -> u32 {
    let proxy = unsafe { &*(this as *const Proxy) };
    proxy.refs.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let proxy = unsafe { &*(this as *const Proxy) };
    let left = proxy.refs.fetch_sub(1, Ordering::AcqRel) - 1;
    if left == 0 {
        unsafe { release_real(proxy.real) };
        unsafe { drop(Box::from_raw(this as *mut Proxy)) };
    }
    left
}
