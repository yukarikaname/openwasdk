//! `CoreUICreate` and the session object the engine talks to through it.
//!
//! `Microsoft.Internal.FrameworkUdk.dll` (`CoreWindowIntegration`) loads the system
//! `coremessaging.dll`, calls `CoreUICreate`, and treats a failure as fatal: the HRESULT
//! goes to `RoFailFastWithErrorContext` and the process dies. Wine carries the export as a
//! stub, so the call has to be answered from this side.
//!
//! Nothing about the object is documented. What is implemented here is what the engine is
//! seen doing, in order, as reported for the same engine build by two independent projects
//! (`WineExplorer`'s Notepad notes and `Penguin-LGHUB`'s `coreui_probe.c`):
//!
//! - the object answers the session interface `{4b2edab9-129b-4733-be15-356853d55ace}`,
//!   whose vtable starts at `IUnknown` -- slot 3 is the message registrar, slot 8 answers
//!   with an item, slot 12 registers a handler;
//! - the item answers `{fcd9d50d-0902-4a2d-ac94-b95c3316f6a0}`.
//!
//! Every other slot refuses with `E_NOTIMPL` and names its number, so the next call the
//! engine makes is visible instead of guessed.

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

use windows_core::GUID;

/// `IMessageSession`: the object `CoreUICreate` returns.
pub(crate) const IID_SESSION: GUID = GUID::from_u128(0x4b2e_dab9_129b_4733_be15_3568_53d5_5ace);

/// The interface the item returned by slot 8 answers.
pub(crate) const IID_ITEM: GUID = GUID::from_u128(0xfcd9_d50d_0902_4a2d_ac94_b95c_3316_f6a0);

/// The interface the message registrar is asked for.
pub(crate) const IID_REGISTRAR: GUID = GUID::from_u128(0x37a9_4c05_1aa6_4cea_9049_083b_ba05_5cec);

const IID_IUNKNOWN: GUID = GUID::from_u128(0x0000_0000_0000_0000_c000_0000_0000_0046);

const S_OK: i32 = 0;
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;

fn traced() -> bool {
    std::env::var_os("OPENCOMP_TRACE").is_some()
}

fn refused(slot: u32) -> i32 {
    if traced() {
        eprintln!("coremessaging: session slot {slot} called");
    }
    E_NOTIMPL
}

/// A method of an undocumented interface. Each is stored as an address, because the
/// interface's methods take different numbers of arguments and nothing here calls them.
type Member = *const c_void;

/// `HRESULT CoreUICreate(void **session)`: one process-wide object, as on Windows.
///
/// # Safety
/// `session` is the one out pointer the caller provided.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn CoreUICreate(session: *mut *mut c_void) -> i32 {
    if session.is_null() {
        return E_POINTER;
    }
    if traced() {
        eprintln!("coremessaging: CoreUICreate");
    }
    unsafe { *session = session_object() };
    S_OK
}

/// The identity is fixed per process, so a second call hands back the same object.
static SESSION: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

fn session_object() -> *mut c_void {
    let existing = SESSION.load(Ordering::Acquire);
    if !existing.is_null() {
        return existing;
    }
    let created = Box::into_raw(Box::new(Object {
        vtbl: &SESSION_VTBL,
        refs: AtomicU32::new(1),
        iids: [IID_SESSION, GUID::from_u128(0)],
    }))
    .cast::<c_void>();
    match SESSION.compare_exchange(
        ptr::null_mut(),
        created,
        Ordering::AcqRel,
        Ordering::Acquire,
    ) {
        Ok(_) => created,
        Err(other) => {
            // Another thread won the race; this thread's object is dropped.
            unsafe { drop(Box::from_raw(created as *mut Object)) };
            other
        }
    }
}

/// Every object here: a reference count, one interface besides `IUnknown`, and a vtable.
#[repr(C)]
struct Object {
    vtbl: *const ObjectVtbl,
    refs: AtomicU32,
    /// The interfaces besides `IUnknown` this object answers; a zero id answers nothing.
    iids: [GUID; 2],
}

#[repr(C)]
struct ObjectVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    /// The interface's own methods, starting at slot 3.
    methods: [Member; 22],
}

// Safety: the table is read-only and every entry is the address of a function.
unsafe impl Sync for ObjectVtbl {}

/// The session's methods: 3 is the registrar, 8 the item, 12 a handler registration.
static SESSION_VTBL: ObjectVtbl = ObjectVtbl {
    query_interface,
    add_ref,
    release,
    methods: [
        get_registrar as Member, // 3
        slot_4 as Member,
        slot_5 as Member,
        slot_6 as Member,
        slot_7 as Member,
        get_item as Member,    // 8
        slot_9 as Member,      // 9
        slot_10 as Member,     // 10
        slot_11 as Member,     // 11
        add_handler as Member, // 12
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
        slot_24 as Member,
    ],
};

/// The registrar and the item: `IUnknown` and four methods that say who was called.
static SMALL_VTBL: ObjectVtbl = ObjectVtbl {
    query_interface,
    add_ref,
    release,
    methods: [
        small_slot_3 as Member,
        small_slot_4 as Member,
        small_slot_5 as Member,
        small_slot_6 as Member,
        slot_unused as Member,
        slot_unused as Member,
        slot_unused as Member,
        slot_unused as Member,
        slot_unused as Member,
        slot_unused as Member,
        slot_unused as Member,
        slot_unused as Member,
        slot_unused as Member,
        slot_unused as Member,
        slot_unused as Member,
        slot_unused as Member,
        slot_unused as Member,
        slot_unused as Member,
        slot_unused as Member,
        slot_unused as Member,
        slot_unused as Member,
        slot_unused as Member,
    ],
};

/// One refusal per unknown session slot, so the trace names the member.
macro_rules! refused_slots {
    ($($slot:expr => $name:ident),* $(,)?) => {
        $(
            unsafe extern "system" fn $name(
                _this: *mut c_void,
                _first: *mut c_void,
                _second: *mut c_void,
            ) -> i32 {
                refused($slot)
            }
        )*
    };
}

refused_slots! {
    4 => slot_4,
    5 => slot_5,
    6 => slot_6,
    7 => slot_7,
    9 => slot_9,
    10 => slot_10,
    11 => slot_11,
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
    24 => slot_24,
}

/// Slot 3: the object that takes message registrations.
unsafe extern "system" fn get_registrar(
    _this: *mut c_void,
    out: *mut c_void,
    _unused: *mut c_void,
) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    if traced() {
        eprintln!("coremessaging: session registrar requested");
    }
    unsafe { *(out as *mut *mut c_void) = small_object([IID_REGISTRAR, GUID::from_u128(0)]) };
    S_OK
}

/// Slot 8: `(argument, out)` -- with a null argument the engine gets an item.
unsafe extern "system" fn get_item(
    _this: *mut c_void,
    _argument: *mut c_void,
    out: *mut c_void,
) -> i32 {
    if out.is_null() {
        return E_POINTER;
    }
    if traced() {
        eprintln!("coremessaging: session item requested with {_argument:?}");
    }
    unsafe { *(out as *mut *mut c_void) = small_object([IID_ITEM, GUID::from_u128(0)]) };
    S_OK
}

/// Slot 12: `(handler, context, object, registration)`. The handler is accepted and the
/// registration answered; nothing delivers to it, because the probe has no message source.
unsafe extern "system" fn add_handler(
    _this: *mut c_void,
    handler: *mut c_void,
    context: *mut c_void,
    registration: *mut c_void,
) -> i32 {
    if traced() {
        eprintln!(
            "coremessaging: session handler {handler:?} context {context:?} registration {registration:?}"
        );
    }
    if registration.is_null() {
        return E_POINTER;
    }
    // The last argument is the registration itself, not a place for one: the caller
    // releases it through the vtable it finds at that address. The object is a struct
    // whose first field is that vtable.
    unsafe {
        *(registration as *mut *const c_void) =
            (&REGISTRATION_VTBL as *const RegistrationVtbl).cast()
    };
    S_OK
}

unsafe extern "system" fn small_slot_3(
    _this: *mut c_void,
    _a: *mut c_void,
    _b: *mut c_void,
) -> i32 {
    if traced() {
        eprintln!("coremessaging: small object slot 3 called");
    }
    E_NOTIMPL
}

unsafe extern "system" fn small_slot_4(
    _this: *mut c_void,
    _a: *mut c_void,
    _b: *mut c_void,
) -> i32 {
    if traced() {
        eprintln!("coremessaging: small object slot 4 called");
    }
    E_NOTIMPL
}

unsafe extern "system" fn small_slot_5(
    _this: *mut c_void,
    _a: *mut c_void,
    _b: *mut c_void,
) -> i32 {
    if traced() {
        eprintln!("coremessaging: small object slot 5 called");
    }
    E_NOTIMPL
}

unsafe extern "system" fn small_slot_6(
    _this: *mut c_void,
    _a: *mut c_void,
    _b: *mut c_void,
) -> i32 {
    if traced() {
        eprintln!("coremessaging: small object slot 6 called");
    }
    E_NOTIMPL
}

/// The registration's `IUnknown`: nothing removes it, and the engine only releases it.
#[repr(C)]
struct RegistrationVtbl {
    query_interface: Member,
    add_ref: Member,
    release: Member,
}

// Safety: read-only table of function addresses.
unsafe impl Sync for RegistrationVtbl {}

static REGISTRATION_VTBL: RegistrationVtbl = RegistrationVtbl {
    query_interface: registration_query_interface as Member,
    add_ref: registration_add_ref as Member,
    release: registration_release as Member,
};

unsafe extern "system" fn registration_query_interface(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if iid.is_null() || out.is_null() {
        return E_POINTER;
    }
    if unsafe { *iid } != IID_IUNKNOWN {
        unsafe { *out = ptr::null_mut() };
        return E_NOINTERFACE;
    }
    unsafe { *out = this };
    S_OK
}

unsafe extern "system" fn registration_add_ref(_this: *mut c_void) -> u32 {
    1
}

unsafe extern "system" fn registration_release(_this: *mut c_void) -> u32 {
    0
}

/// A slot no interface reaches: answering `E_NOTIMPL` is enough for the odd one out.
unsafe extern "system" fn slot_unused(_this: *mut c_void, _a: *mut c_void, _b: *mut c_void) -> i32 {
    if traced() {
        eprintln!("coremessaging: small object past its fourth slot");
    }
    E_NOTIMPL
}

fn small_object(iids: [GUID; 2]) -> *mut c_void {
    Box::into_raw(Box::new(Object {
        vtbl: &SMALL_VTBL,
        refs: AtomicU32::new(1),
        iids,
    }))
    .cast()
}

unsafe extern "system" fn query_interface(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> i32 {
    if iid.is_null() || out.is_null() {
        return E_POINTER;
    }
    let iid = unsafe { *iid };
    let object = unsafe { &*(this as *const Object) };
    let zero = GUID::from_u128(0);
    let known = object
        .iids
        .iter()
        .any(|known| *known != zero && *known == iid);
    if iid == IID_IUNKNOWN || known {
        unsafe { add_ref(this) };
        unsafe { *out = this };
        if traced() {
            eprintln!("coremessaging: object answered {iid:?}");
        }
        return S_OK;
    }
    unsafe { *out = ptr::null_mut() };
    if traced() {
        eprintln!("coremessaging: object asked for {iid:?}");
    }
    E_NOINTERFACE
}

unsafe extern "system" fn add_ref(this: *mut c_void) -> u32 {
    let object = unsafe { &*(this as *const Object) };
    object.refs.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let object = unsafe { &*(this as *const Object) };
    let left = object.refs.fetch_sub(1, Ordering::AcqRel) - 1;
    if left == 0 {
        unsafe { drop(Box::from_raw(this as *mut Object)) };
    }
    left
}
