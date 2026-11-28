//! `{7F3C9B24-EFC1-53A2-A2A9-88280F241AC0}`, the private interface the input layer
//! asks a dispatcher queue for.
//!
//! `Microsoft.UI.Input` takes the thread's queue from
//! `CoreMessagingXP!WinUIGetDispatcherQueueForCurrentThread`, asks it for this
//! interface, and `__fastfail`s when the answer is `E_NOINTERFACE`
//! (`WRL2ContextSession.cpp:219`). With the SDK's own dispatcher the queue object
//! *is* the CoreUI session, so the answer costs nothing; ours is a Win32 message
//! loop, so the answer is a table that starts with the one call the input layer
//! cannot do without.
//!
//! Slot 0 is `Attach(listener, *session)`: `Microsoft.UI.Input` builds a
//! `WRL2ContextSession` object, hands it over, and keeps the session it gets back
//! in its own field `+0xb0`. Everything else reports its own name and refuses.

use core::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use windows_core::{Error, GUID, HRESULT, Result};

const S_OK: HRESULT = HRESULT(0);
const E_NOTIMPL: HRESULT = HRESULT(0x8000_4001u32 as i32);
const E_NOINTERFACE: HRESULT = HRESULT(0x8000_4002u32 as i32);
const E_POINTER: HRESULT = HRESULT(0x8000_4003u32 as i32);

// The IID is not in any metadata: it only appears next to `IDispatcherQueue`'s own
// IID inside the SDK's `CoreMessagingXP.dll`, which is what makes the object a
// session rather than a queue.
windows_core::imp::define_interface!(
    IQueueSession,
    IQueueSession_Vtbl,
    0x7f3c9b24_efc1_53a2_a2a9_88280f241ac0
);

impl windows_core::RuntimeName for IQueueSession {
    const NAME: &'static str = "Microsoft.UI.Dispatching.IQueueSession";
}

fn trace(args: std::fmt::Arguments) {
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!("opendispatch: thread {} {args}", crate::thread::current());
    }
}

fn refuse(slot: &str) -> Result<()> {
    trace(format_args!("queue session {slot} refused"));
    Err(Error::from_hresult(E_NOTIMPL))
}

fn dispatch(slot: &str, _listener: *mut c_void, out: *mut c_void, _fourth: *mut c_void) -> Result<()> {
    match slot {
        "attach" => attach(out),
        other => refuse(other),
    }
}

/// Slot 0's answer: an object with no behaviour of its own.
///
/// The caller stores it, and releases it when its own session object dies; nothing
/// has been seen asking it anything yet. Saying `E_NOTIMPL` instead is what turns
/// into the `__fastfail` in `WRL2ContextSession.cpp:46`.
fn attach(out: *mut c_void) -> Result<()> {
    if out.is_null() {
        return Err(Error::from_hresult(E_POINTER));
    }
    trace(format_args!("queue session attached"));
    let stub = Box::into_raw(Box::new(Stub {
        vtbl: &ATTACHED.0 as *const _ as *const *const c_void,
        count: AtomicU32::new(1),
    }));
    unsafe { *(out as *mut *mut c_void) = stub as *mut c_void };
    Ok(())
}

/// A reference-counted table of `S_OK`. Only the first three slots have meaning.
#[repr(C)]
struct Stub {
    vtbl: *const *const c_void,
    count: AtomicU32,
}

struct Vtbl([*const c_void; SLOTS]);

// The table is immutable function pointers; sharing it is the point.
unsafe impl Sync for Vtbl {}

const SLOTS: usize = 16;

static ATTACHED: Vtbl = {
    let mut table = [stub_call as *const c_void; SLOTS];
    table[0] = stub_query as *const c_void;
    table[1] = stub_addref as *const c_void;
    table[2] = stub_release as *const c_void;
    Vtbl(table)
};

unsafe extern "system" fn stub_query(
    this: *mut c_void,
    iid: *const GUID,
    out: *mut *mut c_void,
) -> HRESULT {
    if iid.is_null() || out.is_null() {
        return E_POINTER;
    }
    let iid = unsafe { *iid };
    trace(format_args!("attached asked for {iid:?}"));
    if iid == <windows_core::IUnknown as windows_core::Interface>::IID
        || iid == <windows_core::IInspectable as windows_core::Interface>::IID
    {
        unsafe { *out = this };
        unsafe { stub_addref(this) };
        return S_OK;
    }
    E_NOINTERFACE
}

unsafe extern "system" fn stub_addref(this: *mut c_void) -> u32 {
    let stub = unsafe { &*(this as *const Stub) };
    stub.count.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn stub_release(this: *mut c_void) -> u32 {
    let stub = unsafe { &*(this as *const Stub) };
    trace(format_args!("attached released, {} left", stub.count.load(Ordering::Relaxed) - 1));
    if stub.count.fetch_sub(1, Ordering::AcqRel) == 1 {
        drop(unsafe { Box::from_raw(this as *mut Stub) });
        return 0;
    }
    stub.count.load(Ordering::Relaxed) - 1
}

unsafe extern "system" fn stub_call(
    _this: *mut c_void,
    _a: *mut c_void,
    _b: *mut c_void,
    _c: *mut c_void,
) -> HRESULT {
    trace(format_args!("attached method called"));
    S_OK
}

/// Declares the interface, its vtable and an implementation that answers every
/// slot by name, so one list is the only place the slot count is written down.
///
/// The ABI is spelled the way `bindings.rs` spells it: the vtable starts with
/// `IInspectable`, and each thunk walks back to the object through `OFFSET`.
macro_rules! session_slots {
    ($target:ty, $($slot:ident),+ $(,)?) => {
        pub trait IQueueSession_Impl: windows_core::IUnknownImpl {
            $(fn $slot(
                &self,
                listener: *mut c_void,
                out: *mut c_void,
                fourth: *mut c_void,
            ) -> Result<()>;)+
        }

        #[repr(C)]
        pub struct IQueueSession_Vtbl {
            pub base__: windows_core::IInspectable_Vtbl,
            $(pub $slot: unsafe extern "system" fn(
                *mut c_void,
                *mut c_void,
                *mut c_void,
                *mut c_void,
            ) -> HRESULT,)+
        }

        impl IQueueSession_Vtbl {
            pub const fn new<Identity: IQueueSession_Impl, const OFFSET: isize>() -> Self {
                $(
                    unsafe extern "system" fn $slot<
                        Identity: IQueueSession_Impl,
                        const OFFSET: isize,
                    >(
                        this: *mut c_void,
                        listener: *mut c_void,
                        out: *mut c_void,
                        fourth: *mut c_void,
                    ) -> HRESULT {
                        unsafe {
                            let this: &Identity =
                                &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                            IQueueSession_Impl::$slot(this, listener, out, fourth).into()
                        }
                    }
                )+
                Self {
                    base__: windows_core::IInspectable_Vtbl::new::<
                        Identity,
                        IQueueSession,
                        OFFSET,
                    >(),
                    $($slot: $slot::<Identity, OFFSET>,)+
                }
            }

            pub fn matches(iid: &GUID) -> bool {
                iid == &<IQueueSession as windows_core::Interface>::IID
            }
        }

        impl IQueueSession_Impl for $target {
            $(fn $slot(
                &self,
                listener: *mut c_void,
                out: *mut c_void,
                fourth: *mut c_void,
            ) -> Result<()> {
                let _ = self;
                dispatch(stringify!($slot), listener, out, fourth)
            })+
        }
    };
}

// Slot 0 is the one call whose shape is known, so it gets a name; the rest are
// numbered until a trace says what reached them.
session_slots!(
    crate::queue_object::QueueObject_Impl,
    attach, op01, op02, op03, op04, op05, op06, op07,
    op08, op09, op10, op11, op12, op13, op14, op15,
    op16, op17, op18, op19, op20, op21, op22, op23,
    op24, op25, op26, op27, op28, op29, op30, op31,
);
