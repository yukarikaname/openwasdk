//! One object that answers both halves of the private statics call.
//!
//! XAML asks the private statics interface for a main device with the
//! `IDCompositionDevice2` IID and then treats the result as a `Microsoft.UI.Composition`
//! compositor. A plain dcomp device answers the first half and crashes at the second;
//! our compositor answers the second and fails the first. This object answers neither
//! itself: `QueryInterface` asks the compositor first, then the device, and hands back
//! whichever one owns the interface.

use std::ffi::c_void;
use std::ptr;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

use windows_core::{GUID, IUnknown, Interface};

const IID_IUNKNOWN: GUID = GUID::from_u128(0x0000_0000_0000_0000_c000_0000_0000_0046);
const IID_IINSPECTABLE: GUID = GUID::from_u128(0xaf86_e2e0_b12d_4c6a_9c5a_d7aa_6510_1e90);
const IID_IAGILEOBJECT: GUID = GUID::from_u128(0x94ea_2b94_e9cc_49e0_c0ff_ee64_ca8f_5b90);

const S_OK: i32 = 0;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;

type QueryInterface = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
type AddRef = unsafe extern "system" fn(*mut c_void) -> u32;
type Release = unsafe extern "system" fn(*mut c_void) -> u32;

#[repr(C)]
struct Composite {
    vtbl: *const Vtbl,
    refs: AtomicU32,
    compositor: IUnknown,
    device: *mut c_void,
}

#[repr(C)]
struct Vtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
}

static VTBL: Vtbl = Vtbl {
    query_interface,
    add_ref,
    release,
};

/// Wrap `compositor` and `device` into the object the statics slot hands out. The device
/// reference moves into the object, and is released with it.
pub fn create(compositor: IUnknown, device: *mut c_void) -> *mut c_void {
    let composite = Box::new(Composite {
        vtbl: &VTBL,
        refs: AtomicU32::new(1),
        compositor,
        device,
    });
    Box::into_raw(composite).cast()
}



/// A refusal with a name: every slot logs which one was called and reports E_NOTIMPL.
/// Returning this instead of E_NOINTERFACE keeps the caller alive long enough to say what
/// it wanted, which is the only way to learn an interface nothing documents.
fn stub(iid: GUID) -> *mut c_void {
    /// The answer to an unknown interface. Its 0x50 field carries the creating thread id:
    /// XAML's callers check an object's affinity by comparing GetCurrentThreadId() with the
    /// dword at that offset, and reading it out of a smaller allocation was the fault.
    #[repr(C)]
    struct Stub {
        vtbl: *const StubVtbl,
        refs: AtomicU32,
        iid: GUID,
        pad: [u8; 0x50 - 0x1c],
        thread: u32,
        tail: [u8; 0xc],
    }
    type Slot = unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> i32;
    #[repr(C)]
    struct StubVtbl {
        query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32,
        add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        slots: [Slot; 24],
    }
    const REFUSE: i32 = 0x8000_4001u32 as i32;
    unsafe extern "system" fn stub_qi(this: *mut c_void, iid: *const GUID, out: *mut *mut c_void) -> i32 {
        if iid.is_null() || out.is_null() {
            return E_POINTER;
        }
        let iid = unsafe { *iid };
        if iid == IID_IUNKNOWN || iid == IID_IINSPECTABLE {
            unsafe { stub_add_ref(this) };
            unsafe { *out = this };
            return S_OK;
        }
        unsafe { *out = ptr::null_mut() };
        E_NOINTERFACE
    }
    unsafe extern "system" fn stub_add_ref(this: *mut c_void) -> u32 {
        let stub = unsafe { &*(this as *const Stub) };
        stub.refs.fetch_add(1, Ordering::Relaxed) + 1
    }
    unsafe extern "system" fn stub_release(this: *mut c_void) -> u32 {
        let stub = unsafe { &*(this as *const Stub) };
        let refs = stub.refs.fetch_sub(1, Ordering::Release) - 1;
        if refs == 0 {
            std::sync::atomic::fence(Ordering::Acquire);
            drop(unsafe { Box::from_raw(this as *mut Stub) });
        }
        refs
    }
    unsafe extern "system" fn stub_slot<const N: usize>(
        this: *mut c_void,
        _a: *mut c_void,
        _b: *mut c_void,
        _c: *mut c_void,
    ) -> i32 {
        let stub = unsafe { &*(this as *const Stub) };
        if std::env::var_os("OPENCOMP_TRACE").is_some() {
            eprintln!("opencomp: stub for {:?} called at slot {N}", stub.iid);
            crate::private_statics::trace_stack();
        }
        // Whether these methods have to do anything is unknown; OPENCOMP_STUB_OK answers
        // them successfully and shows how far that carries, which is cheaper than
        // guessing at each one.
        if std::env::var_os("OPENCOMP_STUB_OK").is_some() {
            return S_OK;
        }
        REFUSE
    }
    static VTBLS: OnceLock<StubVtbl> = OnceLock::new();
    let vtbl = VTBLS.get_or_init(|| StubVtbl {
        query_interface: stub_qi,
        add_ref: stub_add_ref,
        release: stub_release,
        slots: [
            stub_slot::<0>, stub_slot::<1>, stub_slot::<2>, stub_slot::<3>,
            stub_slot::<4>, stub_slot::<5>, stub_slot::<6>, stub_slot::<7>,
            stub_slot::<8>, stub_slot::<9>, stub_slot::<10>, stub_slot::<11>,
            stub_slot::<12>, stub_slot::<13>, stub_slot::<14>, stub_slot::<15>,
            stub_slot::<16>, stub_slot::<17>, stub_slot::<18>, stub_slot::<19>,
            stub_slot::<20>, stub_slot::<21>, stub_slot::<22>, stub_slot::<23>,
        ],
    });
    let boxed = Box::new(Stub {
        vtbl,
        refs: AtomicU32::new(1),
        iid,
        pad: [0; 0x50 - 0x1c],
        thread: current_thread_id(),
        tail: [0; 0xc],
    });
    Box::into_raw(boxed).cast()
}

/// The id of the thread that is building the stub, for the affinity field above.
fn current_thread_id() -> u32 {
    #[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
    unsafe extern "system" {
        fn GetCurrentThreadId() -> u32;
    }
    unsafe { GetCurrentThreadId() }
}

/// Ask one raw object for an interface, using its own vtable.
unsafe fn ask(object: *mut c_void, iid: &GUID, out: *mut *mut c_void) -> Option<i32> {
    if object.is_null() {
        return None;
    }
    let vtbl = unsafe { *(object as *const *const usize) };
    if vtbl.is_null() {
        return None;
    }
    let query: QueryInterface = unsafe { std::mem::transmute(*vtbl) };
    let hr = unsafe { query(object, iid, out) };
    if hr >= 0 { Some(hr) } else { None }
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
    if iid == IID_IUNKNOWN || iid == IID_IINSPECTABLE || iid == IID_IAGILEOBJECT {
        unsafe { add_ref(this) };
        unsafe { *out = this };
        return S_OK;
    }
    let composite = unsafe { &*(this as *const Composite) };
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!("opencomp: composite asked for {iid:?}");
    }
    let compositor = Interface::as_raw(&composite.compositor);
    if unsafe { ask(compositor, &iid, out) }.is_some() {
        return S_OK;
    }
    if unsafe { ask(composite.device, &iid, out) }.is_some() {
        return S_OK;
    }
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!("opencomp: composite has neither the compositor nor the device for {iid:?}");
    }
    // XAML asks the object for interfaces this pair does not carry and fails fast when it
    // is refused. Answer with a stub that names the slot it calls, so the next round knows
    // what the interface has to do.
    unsafe { *out = stub(iid) };
    S_OK
}

unsafe extern "system" fn add_ref(this: *mut c_void) -> u32 {
    let composite = unsafe { &*(this as *const Composite) };
    composite.refs.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn release(this: *mut c_void) -> u32 {
    let composite = unsafe { &*(this as *const Composite) };
    let refs = composite.refs.fetch_sub(1, Ordering::Release) - 1;
    if refs == 0 {
        std::sync::atomic::fence(Ordering::Acquire);
        let mut owned = unsafe { Box::from_raw(this as *mut Composite) };
        if !owned.device.is_null() {
            let object: IUnknown = unsafe { IUnknown::from_raw(owned.device) };
            drop(object);
            owned.device = ptr::null_mut();
        }
        drop(owned);
    }
    refs
}
