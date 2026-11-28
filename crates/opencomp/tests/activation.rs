#![cfg(windows)]

//! Activation through the Windows runtime, not through our export.
//!
//! A WinUI application never calls `DllGetActivationFactory`: it calls
//! `RoActivateInstance` or `RoGetActivationFactory` with a class name, and combase
//! finds the DLL through a registration. Two registrations can exist in a process
//! and this file tests both:
//!
//! - an activation context built from the manifest an application ships, which is
//!   the route a packaged or unpackaged application takes and the one that works
//!   on Windows and under Wine;
//! - `RoRegisterActivationFactories`, which the crate exposes as
//!   `opencomp::register`. The runtime documents it for an exe server whose
//!   classes are registered as out-of-process and may refuse it for a plain
//!   client, so the test asserts the path only when the runtime accepts it.
//!
//! The calls below are made here rather than through the crate's wrappers, so the
//! test does not depend on them. Nothing here calls the export.

#![cfg(windows)]

use std::ffi::c_void;
use std::fs;
use std::path::PathBuf;

use opencomp::bindings::Microsoft::UI::Composition::{ICompositor, IVisual};
use windows_core::{GUID, HRESULT, HSTRING, IInspectable, Interface};
use windows_numerics::Vector2;

const COMPOSITOR: &str = "Microsoft.UI.Composition.Compositor";
const IID_IACTIVATION_FACTORY: GUID = GUID::from_u128(0x0000_0035_0000_0000_c000_0000_0000_0046);
const RPC_E_CHANGED_MODE: i32 = 0x8001_0106u32 as i32;
const INVALID_HANDLE_VALUE: *mut c_void = (-1isize) as *mut c_void;

/// `ACTCTXW`, the descriptor `CreateActCtxW` takes.
#[repr(C)]
struct ActCtx {
    size: u32,
    flags: u32,
    source: *const u16,
    architecture: u16,
    language: u16,
    assembly_directory: *const u16,
    resource_name: *const u16,
    application_name: *const u16,
    module: *mut c_void,
}

windows_core::link!("api-ms-win-core-winrt-l1-1-0.dll" "system" fn RoInitialize(kind: i32) -> HRESULT);
windows_core::link!("api-ms-win-core-winrt-l1-1-0.dll" "system" fn RoActivateInstance(
    class: *mut c_void,
    instance: *mut *mut c_void,
) -> HRESULT);
windows_core::link!("api-ms-win-core-winrt-l1-1-0.dll" "system" fn RoGetActivationFactory(
    class: *mut c_void,
    iid: *const GUID,
    factory: *mut *mut c_void,
) -> HRESULT);
windows_core::link!("kernel32.dll" "system" fn CreateActCtxW(context: *const ActCtx) -> *mut c_void);
windows_core::link!("kernel32.dll" "system" fn ActivateActCtx(
    context: *mut c_void,
    cookie: *mut usize,
) -> i32);
windows_core::link!("kernel32.dll" "system" fn DeactivateActCtx(flags: u32, cookie: usize) -> i32);
windows_core::link!("kernel32.dll" "system" fn ReleaseActCtx(context: *mut c_void));
windows_core::link!("kernel32.dll" "system" fn GetModuleHandleW(name: *const u16) -> *mut c_void);
windows_core::link!("kernel32.dll" "system" fn GetProcAddress(
    module: *mut c_void,
    name: *const u8,
) -> *mut c_void);
windows_core::link!("kernel32.dll" "system" fn GetLastError() -> u32);

/// Enter an apartment; activation needs one and a host may have chosen already.
fn initialize() {
    let result = unsafe { RoInitialize(1) }; // RO_INIT_MULTITHREADED
    assert!(
        result.0 >= 0 || result.0 == RPC_E_CHANGED_MODE,
        "RoInitialize = {:#x}",
        result.0
    );
}

/// Wine replaces combase, and its registration entry point keeps nothing. Ask
/// ntdll, which is where it admits to being Wine, instead of guessing from a
/// failed activation.
fn under_wine() -> bool {
    // Safety: both names are NUL terminated and live across the calls.
    unsafe {
        let ntdll = GetModuleHandleW(wide("ntdll.dll").as_ptr());
        !ntdll.is_null() && !GetProcAddress(ntdll, c"wine_get_version".as_ptr().cast()).is_null()
    }
}

fn wide(name: &str) -> Vec<u16> {
    name.encode_utf16().chain(Some(0)).collect()
}

fn handle(name: &HSTRING) -> *mut c_void {
    unsafe { std::mem::transmute_copy(name) }
}

/// Activate the compositor the way an application does, and hand back the object.
fn activate() -> windows_core::Result<IInspectable> {
    let name = HSTRING::from(COMPOSITOR);
    let mut raw: *mut c_void = std::ptr::null_mut();
    // Safety: `name` is live for the call and `raw` is writable.
    let result = unsafe { RoActivateInstance(handle(&name), &mut raw) };
    if result.0 < 0 {
        return Err(windows_core::Error::from_hresult(result));
    }
    // Safety: a successful activation hands over one reference.
    Ok(unsafe { IInspectable::from_raw(raw) })
}

/// The factory the runtime holds for the class, which is the statics route.
fn factory() -> windows_core::Result<windows_core::imp::IGenericFactory> {
    let name = HSTRING::from(COMPOSITOR);
    let mut raw: *mut c_void = std::ptr::null_mut();
    // Safety: `name` is live for the call, `raw` is writable, and the IID asks for
    // the activation factory itself, which is what a class factory is.
    let result =
        unsafe { RoGetActivationFactory(handle(&name), &IID_IACTIVATION_FACTORY, &mut raw) };
    if result.0 < 0 {
        return Err(windows_core::Error::from_hresult(result));
    }
    // Safety: a successful lookup hands over one reference to the factory.
    Ok(unsafe { Interface::from_raw(raw) })
}

/// The runtime really activated this DLL, not a Microsoft runtime the machine may
/// have registered: objects this DLL makes are in the node table it publishes.
fn assert_our_compositor(instance: IInspectable) {
    let compositor: ICompositor = instance.cast().expect("ICompositor");
    let sprite = compositor.CreateSpriteVisual().expect("CreateSpriteVisual");
    assert!(
        opencomp::node::resolve(&sprite).is_some(),
        "the runtime activated a compositor that is not ours"
    );
    let visual: IVisual = sprite.cast().expect("IVisual");
    visual.SetSize(Vector2 { x: 4.0, y: 2.0 }).expect("SetSize");
}

/// The directory the test executable runs from, after making sure `opencomp.dll`
/// is in it: `LoadLibraryW` searches the application directory first, and the
/// manifest names the DLL by file name alone.
///
/// Cargo runs the tests before it builds the cdylib, so the DLL is either beside
/// the executable (the CI order) or one directory up (built first). A run with no
/// DLL stands down with a message instead of pretending to test activation.
fn install_dll() -> Option<PathBuf> {
    let directory = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let beside = directory.join("opencomp.dll");
    if beside.exists() {
        return Some(directory);
    }
    let built = std::env::var_os("OPENCOMP_DLL")
        .map(PathBuf::from)
        .or_else(|| {
            let up = directory.join("..").join("opencomp.dll");
            up.exists().then_some(up)
        })?;
    fs::copy(&built, &beside).ok()?;
    Some(directory)
}

/// The manifest an application ships, for one class and our DLL.
///
/// `xwinui manifest` writes the full one; this is the same shape, and the
/// `activatableClass` element has to be in the WinRT namespace for the runtime to
/// read it. The copy exists because this test cannot depend on the host tool,
/// which does not build for Windows.
fn manifest() -> String {
    let mut text = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    text.push_str(
        "<assembly manifestVersion=\"1.0\" xmlns=\"urn:schemas-microsoft-com:asm.v1\">\n",
    );
    text.push_str("  <assemblyIdentity version=\"1.0.0.0\" name=\"opencomp.activation.test\" />\n");
    text.push_str("  <file name=\"opencomp.dll\">\n");
    text.push_str(&format!(
        "    <activatableClass name=\"{COMPOSITOR}\" threadingModel=\"both\" xmlns=\"urn:schemas-microsoft-com:winrt.v1\" />\n"
    ));
    text.push_str("  </file>\n");
    text.push_str("</assembly>\n");
    text
}

#[test]
fn a_manifest_lets_the_runtime_load_the_dll() {
    initialize();
    let Some(directory) = install_dll() else {
        eprintln!(
            "activation: opencomp.dll is not next to the test executable; build the DLL first \
             to run the manifest route"
        );
        return;
    };
    let path = directory.join("opencomp-activation.manifest");
    fs::write(&path, manifest()).expect("write manifest");

    let source = wide(&path.to_string_lossy());
    let context = ActCtx {
        size: std::mem::size_of::<ActCtx>() as u32,
        flags: 0,
        source: source.as_ptr(),
        architecture: 0,
        language: 0,
        assembly_directory: std::ptr::null(),
        resource_name: std::ptr::null(),
        application_name: std::ptr::null(),
        module: std::ptr::null_mut(),
    };
    // Safety: the descriptor points at `path`, which outlives the call.
    let handle = unsafe { CreateActCtxW(&context) };
    let error = unsafe { GetLastError() };
    assert_ne!(
        handle, INVALID_HANDLE_VALUE,
        "CreateActCtxW failed: {error}"
    );
    let mut cookie: usize = 0;
    // Safety: `handle` came from CreateActCtxW and `cookie` is writable.
    let activated = unsafe { ActivateActCtx(handle, &mut cookie) };
    let error = unsafe { GetLastError() };
    assert_ne!(activated, 0, "ActivateActCtx failed: {error}");

    let instance = activate().expect("RoActivateInstance through the manifest");
    assert_our_compositor(instance);

    // Safety: the token and the handle are the ones created above.
    unsafe {
        DeactivateActCtx(0, cookie);
        ReleaseActCtx(handle);
    }
}

/// The in-process route the crate offers a host: register, then activate.
///
/// `RoRegisterActivationFactories` is documented for an exe server whose classes
/// are registered as out-of-process, so a runtime may refuse it for a plain
/// client. When it does, or when the runtime is Wine and stubs the call, the test
/// says which of the two happened and leaves the path unasserted; the manifest
/// test above is the one that must pass.
#[test]
fn an_accepted_registration_serves_the_compositor() {
    initialize();
    if let Err(error) = opencomp::register() {
        if opencomp::unregistered(error.code()) {
            eprintln!(
                "activation: this runtime refused the in-process registration ({:#010x}); the \
                 manifest route is the one a host has",
                error.code().0
            );
            return;
        }
        panic!("RoRegisterActivationFactories = {:#010x}", error.code().0);
    }

    let instance = match activate() {
        Ok(instance) => instance,
        Err(error) if opencomp::unregistered(error.code()) && under_wine() => {
            eprintln!(
                "activation: Wine's combase stubs RoRegisterActivationFactories, so the \
                 in-process route cannot work there; it is asserted on Windows"
            );
            return;
        }
        Err(error) => panic!("RoActivateInstance = {:#010x}", error.code().0),
    };
    assert_our_compositor(instance);

    // The factory route has to resolve too: a class with statics is reached
    // through it, and the runtime casts to the IID it was asked for.
    let instance: IInspectable = factory()
        .expect("RoGetActivationFactory")
        .ActivateInstance()
        .expect("ActivateInstance through the factory");
    assert_our_compositor(instance);
}

#[test]
fn registering_twice_returns_the_first_answer() {
    initialize();
    let first = opencomp::register().is_ok();
    let second = opencomp::register().is_ok();
    assert_eq!(first, second, "register has to be idempotent");
}
