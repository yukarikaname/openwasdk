//! Gate B's entry point: start the engine and see whether it reaches our callback.
//!
//! Split from `windows.rs` to keep both files readable. Everything here talks to the
//! engine through the DLL's own exports and the generated bindings; the pieces it needs
//! from the rest of the probe are the loader and the export lookup.

use std::ffi::c_void;
use std::path::PathBuf;
use std::ptr;

use windows_core::{HSTRING, IUnknown, Interface};
use windows_sys::Win32::Foundation::HMODULE;

use crate::windows::{export, handle, load};
use crate::xaml_bindings::Microsoft::UI::Xaml::{
    ApplicationInitializationCallback, IApplicationStatics,
};

// XAML installs a dispatcher on the calling thread and expects it to be an apartment.
// A process that never called `RoInitialize` gets whatever the engine picks.
#[link(name = "combase.dll", kind = "raw-dylib", modifiers = "+verbatim")]
unsafe extern "system" {
    fn RoInitialize(init_type: i32) -> i32;
}

/// `IDispatcherQueueControllerStatics`, in metadata order.
///
/// `comptrace interfaces Microsoft.UI.winmd --namespace Microsoft.UI.Dispatching` gives
/// the IID, and `comptrace methods --interface IDispatcherQueueControllerStatics` gives
/// the order: `CreateOnDedicatedThread` first, `CreateOnCurrentThread` second.
#[repr(C)]
struct DispatcherStaticsVtbl {
    base__: windows_core::IInspectable_Vtbl,
    create_on_dedicated_thread: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
    create_on_current_thread: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
}

#[repr(transparent)]
#[derive(Clone)]
struct DispatcherStatics(IUnknown);

unsafe impl Interface for DispatcherStatics {
    type Vtable = DispatcherStaticsVtbl;
    const IID: windows_core::GUID =
        windows_core::GUID::from_u128(0xf18d_6145_722b_593d_bcf2_a61e_713f_0037);
}

/// The activation factory of `class`, through the DLL's own export rather than combase:
/// the point is to test this DLL, not the registration of the machine.
fn class_factory(module: HMODULE, class: &str) -> Option<IUnknown> {
    let get_factory = export(module, "DllGetActivationFactory")?;
    // `HRESULT DllGetActivationFactory(HSTRING, IActivationFactory **)`.
    let get_factory: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32 =
        unsafe { std::mem::transmute(get_factory) };
    let name = HSTRING::from(class);
    let mut raw: *mut c_void = ptr::null_mut();
    let hr = unsafe { get_factory(handle(&name), &mut raw) };
    if hr < 0 || raw.is_null() {
        eprintln!(
            "xaml-probe: DllGetActivationFactory({class}) hr=0x{:08x}",
            hr as u32
        );
        return None;
    }
    Some(unsafe { IUnknown::from_raw(raw) })
}
/// Create the dispatcher `Application.Start` needs on the calling thread.
///
/// Start does not make one: `Microsoft.UI.Dispatching.DispatcherQueueController` is
/// served by `Microsoft.UI.dll`, not by the XAML DLL, which answers
/// `CLASS_E_CLASSNOTAVAILABLE` for it. An engine that starts without a queue on its
/// thread waits for one that never delivers.
fn create_dispatcher(module: HMODULE) -> Option<i32> {
    let factory = class_factory(module, "Microsoft.UI.Dispatching.DispatcherQueueController")?;
    let statics: DispatcherStatics = match factory.cast() {
        Ok(statics) => statics,
        Err(error) => {
            eprintln!(
                "xaml-probe: QueryInterface(IDispatcherQueueControllerStatics) hr=0x{:08x}",
                error.code().0 as u32
            );
            return None;
        }
    };
    let mut controller: *mut c_void = ptr::null_mut();
    let hr = unsafe {
        (Interface::vtable(&statics).create_on_current_thread)(
            Interface::as_raw(&statics),
            &mut controller,
        )
    };
    if hr >= 0 && !controller.is_null() {
        // The probe keeps it for the process lifetime: this is the controller the
        // engine is about to look for.
        let _ = unsafe { IUnknown::from_raw(controller) };
    }
    Some(hr)
}

/// Gate B's first measurement: hand `Application.Start` a callback and see whether the
/// engine runs it on the thread it owns. Nothing below that -- a tree, a measure, a
/// frame -- can happen until this does.
///
/// The call goes through the generated `IApplicationStatics::Start`, not a hand-written
/// slot: the delegate ABI is part of the call, and a wrong slot returns `S_OK` without
/// running anything, which is exactly what the hand-written probe did on both hosts.
pub(crate) fn run(module: HMODULE) -> i32 {
    // Safety: the apartment is process state and this is the first COM call the probe makes.
    let hr = unsafe { RoInitialize(0) };
    eprintln!("xaml-probe: RoInitialize(STA) hr=0x{:08x}", hr as u32);
    let Some(get_factory) = export(module, "DllGetActivationFactory") else {
        eprintln!("xaml-probe: DllGetActivationFactory missing");
        return 1;
    };
    // `HRESULT DllGetActivationFactory(HSTRING, IActivationFactory **)`.
    let get_factory: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32 =
        unsafe { std::mem::transmute(get_factory) };
    let name = HSTRING::from("Microsoft.UI.Xaml.Application");
    let mut raw: *mut c_void = ptr::null_mut();
    let hr = unsafe { get_factory(handle(&name), &mut raw) };
    if hr < 0 || raw.is_null() {
        eprintln!(
            "xaml-probe: DllGetActivationFactory(Application) hr=0x{:08x}",
            hr as u32
        );
        return 1;
    }
    // `Microsoft.UI.Dispatching.DispatcherQueueController` is served by
    // `CoreMessagingXP.dll` -- the SDK's activation table says so, and asking the XAML DLL
    // or `Microsoft.UI.dll` for it answers `CLASS_E_CLASSNOTAVAILABLE`.
    let messaging = std::env::var_os("XAML_DLL")
        .map(|path| PathBuf::from(path).with_file_name("CoreMessagingXP.dll"))
        .and_then(|path| load(&path));
    match messaging.map(create_dispatcher) {
        Some(Some(hr)) => eprintln!(
            "xaml-probe: DispatcherQueueController.CreateOnCurrentThread hr=0x{:08x}",
            hr as u32
        ),
        Some(None) => eprintln!("xaml-probe: no dispatcher controller"),
        None => eprintln!("xaml-probe: CoreMessagingXP.dll not loadable"),
    }
    let factory: IUnknown = unsafe { IUnknown::from_raw(raw) };
    let Ok(statics) = factory.cast::<IApplicationStatics>() else {
        eprintln!("xaml-probe: no IApplicationStatics on the factory");
        return 1;
    };

    let callback = ApplicationInitializationCallback::new(|_params| {
        // The engine called us on its own thread with a core behind it. Everything the
        // gate needs -- a tree, a measure -- starts from here.
        // stderr, not stdout: a run that is killed by its timeout loses a buffered
        // line, and this is the line that decides whether the engine started.
        eprintln!("xaml-probe: Application.Start called back");
        std::process::exit(0);
    });
    println!("{{\"kind\":\"start\",\"state\":\"calling\"}}");

    // The generated vtable names the slot and the generated delegate carries the ABI;
    // `--minimal` leaves out only the method wrapper, which is this one line.
    let hr = unsafe {
        (Interface::vtable(&statics).Start)(
            Interface::as_raw(&statics),
            Interface::as_raw(&callback),
        )
    };
    // A start that returns without calling back never built a core.
    println!(
        "{{\"kind\":\"start\",\"state\":\"returned\",\"hr\":\"0x{:08x}\"}}",
        hr.0 as u32
    );
    1
}
