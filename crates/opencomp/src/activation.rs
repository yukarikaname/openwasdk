//! Activation the way an application reaches it: through the Windows runtime.
//!
//! An application never calls [`crate::DllGetActivationFactory`]. It calls
//! `RoActivateInstance` or `RoGetActivationFactory` with a class name, and combase
//! has to know which DLL answers for that name. A packaged or unpackaged
//! application ships that knowledge as a manifest. A host that loads this DLL
//! itself can instead call [`register`], which asks
//! `RoRegisterActivationFactories` to keep the mapping in the process — a call the
//! runtime documents for out-of-process exe servers, so a host it refuses ships
//! the manifest like an application does.

use std::ffi::c_void;
use std::sync::OnceLock;

use windows_core::{Error, GUID, HRESULT, HSTRING, IInspectable, Interface};

use crate::CLASSES;

const S_OK: HRESULT = HRESULT(0);
const E_POINTER: HRESULT = HRESULT(0x8000_4003u32 as i32);
const CLASS_E_CLASSNOTAVAILABLE: HRESULT = HRESULT(0x8004_0111u32 as i32);
const REGDB_E_READREGDB: i32 = 0x8004_0150u32 as i32;
const REGDB_E_KEYMISSING: i32 = 0x8004_0152u32 as i32;
const REGDB_E_CLASSNOTREG: i32 = 0x8004_0154u32 as i32;

/// One callback per registered class: `HRESULT (*)(HSTRING, IActivationFactory **)`.
///
/// The HSTRING is passed as its handle, which is what `HSTRING` is a wrapper
/// around, so the type stays a pointer here like the other exports in this crate.
type ActivationFactoryCallback =
    unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT;

windows_core::link!("api-ms-win-core-winrt-l1-1-0.dll" "system" fn RoRegisterActivationFactories(
    classes: *const *mut c_void,
    callbacks: *const ActivationFactoryCallback,
    count: u32,
    cookie: *mut *mut c_void,
) -> HRESULT);
windows_core::link!("api-ms-win-core-winrt-l1-1-0.dll" "system" fn RoActivateInstance(
    class: *mut c_void,
    instance: *mut *mut c_void,
) -> HRESULT);
windows_core::link!("api-ms-win-core-winrt-l1-1-0.dll" "system" fn RoGetActivationFactory(
    class: *mut c_void,
    iid: *const GUID,
    factory: *mut *mut c_void,
) -> HRESULT);

/// Whether `code` means the runtime has no registration for the class.
///
/// combase reports a class it cannot place as `REGDB_E_CLASSNOTREG`; a runtime
/// with no activation registry section at all, as in a fresh Wine prefix, reports
/// `REGDB_E_READREGDB`, and a manifest entry that names no DLL reports
/// `CLASS_E_CLASSNOTAVAILABLE`. A host that can fall back treats all three alike.
pub fn unregistered(code: HRESULT) -> bool {
    code == CLASS_E_CLASSNOTAVAILABLE
        || code.0 == REGDB_E_READREGDB
        || code.0 == REGDB_E_KEYMISSING
        || code.0 == REGDB_E_CLASSNOTREG
}

/// Register every class in [`CLASSES`] with the runtime, in-process.
///
/// After this call combase answers `RoGetActivationFactory` and
/// `RoActivateInstance` for these names by calling this DLL, with no manifest
/// involved. The first call registers the classes and every later call returns
/// its result, so a host may call it from each startup path it has.
///
/// The runtime documents this function for the exe server of an out-of-process
/// class, and can answer `REGDB_E_CLASSNOTREG` for a class that is not registered
/// that way. A host that gets that error needs the manifest route instead: it is
/// what an application ships, and what `opencomp-client` falls back to.
pub fn register() -> windows_core::Result<()> {
    static REGISTERED: OnceLock<HRESULT> = OnceLock::new();
    let result = *REGISTERED.get_or_init(register_all);
    if result.0 < 0 {
        return Err(Error::from_hresult(result));
    }
    Ok(())
}

fn register_all() -> HRESULT {
    let names: Vec<HSTRING> = CLASSES.iter().map(|name| HSTRING::from(*name)).collect();
    let mut classes: Vec<*mut c_void> = names.iter().map(handle).collect();
    let callbacks: Vec<ActivationFactoryCallback> = vec![activation_factory; CLASSES.len()];
    let mut cookie: *mut c_void = std::ptr::null_mut();
    // Safety: both arrays hold one entry per class and outlive the call; combase
    // copies the names it keeps, and the cookie is only needed to revoke.
    unsafe {
        RoRegisterActivationFactories(
            classes.as_mut_ptr(),
            callbacks.as_ptr(),
            CLASSES.len() as u32,
            &mut cookie,
        )
    }
}

/// Activate `class` the way an application does.
///
/// The returned object carries one reference; the caller casts it to the
/// interface it wants.
pub fn activate(class: &str) -> windows_core::Result<IInspectable> {
    let name = HSTRING::from(class);
    let mut raw: *mut c_void = std::ptr::null_mut();
    // Safety: `name` is live across the call and `raw` is writable.
    let result = unsafe { RoActivateInstance(handle(&name), &mut raw) };
    if result.0 < 0 {
        return Err(Error::from_hresult(result));
    }
    // Safety: a successful activation hands over the one reference this wraps.
    Ok(unsafe { IInspectable::from_raw(raw) })
}

/// The activation factory for `class`, already cast to `I`.
///
/// A class with no default constructor is reached through its factory, so this is
/// the runtime-side counterpart of asking the DLL for the factory directly.
pub fn factory_of<I: Interface>(class: &str) -> windows_core::Result<I> {
    let name = HSTRING::from(class);
    let mut raw: *mut c_void = std::ptr::null_mut();
    // Safety: `name` is live across the call and `raw` is writable.
    let result = unsafe { RoGetActivationFactory(handle(&name), &I::IID, &mut raw) };
    if result.0 < 0 {
        return Err(Error::from_hresult(result));
    }
    // Safety: the runtime returns the factory with one reference for `I::IID`.
    Ok(unsafe { I::from_raw(raw) })
}

/// The factory combase asks for by class name.
///
/// # Safety
/// `class` is the HSTRING handle combase owns for the duration of the call and
/// `factory` is writable for one `IActivationFactory` pointer.
unsafe extern "system" fn activation_factory(
    class: *mut c_void,
    factory: *mut *mut c_void,
) -> HRESULT {
    if class.is_null() || factory.is_null() {
        return E_POINTER;
    }
    // The handle is the pointer inside the string, so bit copy it and keep the
    // caller's reference from being released here.
    let name = std::mem::ManuallyDrop::new(unsafe {
        std::mem::transmute_copy::<*mut c_void, HSTRING>(&class)
    });
    let Some(selected) = crate::class_of(&name.to_string_lossy()) else {
        unsafe { *factory = std::ptr::null_mut() };
        return CLASS_E_CLASSNOTAVAILABLE;
    };
    unsafe { *factory = crate::factory::Factory::create(selected) };
    S_OK
}

/// The HSTRING handle, which is the pointer inside the string header.
fn handle(name: &HSTRING) -> *mut c_void {
    unsafe { std::mem::transmute_copy(name) }
}
