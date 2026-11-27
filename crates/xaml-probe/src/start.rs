//! Gate B's entry point: start the engine and measure one layout case in its callback.
//!
//! Split from `windows.rs` to keep both files readable. The loader, the export lookup and
//! the activation factory come from there; everything below the callback talks through the
//! generated bindings, so a slot the engine disagrees with is the generator's problem
//! rather than a number written by hand.

use std::ffi::c_void;
use std::io::Write;
use std::ptr;

use windows_core::{HSTRING, IUnknown, Interface};
use windows_sys::Win32::Foundation::HMODULE;

use crate::windows::{IActivationFactory, export, handle};
use crate::xaml_bindings::Microsoft::UI::Xaml::Controls::UIElementCollection;
use crate::xaml_bindings::Microsoft::UI::Xaml::Controls::{IStackPanel, IStackPanelFactory};
use crate::xaml_bindings::Microsoft::UI::Xaml::{
    ApplicationInitializationCallback, IApplicationFactory, IApplicationStatics, IFrameworkElement,
    IUIElement, UIElement,
};
use crate::xaml_bindings::Windows::Foundation::{Rect, Size};

pub(crate) const E_POINTER: i32 = 0x8000_4003u32 as i32;

/// The layer `Microsoft.ui.xaml.dll` reaches its input core through.
const UDK: &str = "Microsoft.Internal.FrameworkUdk.dll";
/// The messaging DLL the input core is created from: the App SDK's own.
const MESSAGING: &str = "CoreMessagingXP.dll";
/// Our stand-in, used only when `OPENCOMP_COREUI=own` asks for it.
const OWN_MESSAGING: &str = "opencoremsg.dll";

pub(crate) type GetActivationFactory =
    unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
type CreateInstance = unsafe extern "system" fn(
    *mut c_void,
    *mut c_void,
    *mut *mut c_void,
    *mut *mut c_void,
) -> windows_core::HRESULT;

// XAML installs a dispatcher on the calling thread and expects it to be an apartment.
// A process that never called `RoInitialize` gets whatever the engine picks.
#[link(name = "kernel32.dll", kind = "raw-dylib", modifiers = "+verbatim")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn TerminateProcess(process: *mut c_void, code: u32) -> i32;
}

#[link(name = "combase.dll", kind = "raw-dylib", modifiers = "+verbatim")]
unsafe extern "system" {
    fn RoInitialize(init_type: i32) -> i32;
    fn RoGetActivationFactory(
        class: *mut c_void,
        iid: *const windows_core::GUID,
        out: *mut *mut c_void,
    ) -> i32;
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

/// Create the dispatcher `Application.Start` needs on the calling thread.
///
/// The class is reached through **combase**, which is how an application reaches it and
/// how the activation manifest decides which DLL serves it -- in our case
/// `opendispatch.dll` instead of the SDK's `CoreMessagingXP.dll`, which aborts on load
/// under Wine. Asking a DLL by name was the earlier mistake here.
fn create_dispatcher() -> Option<i32> {
    let name = HSTRING::from("Microsoft.UI.Dispatching.DispatcherQueueController");
    let mut statics: *mut c_void = ptr::null_mut();
    let hr =
        unsafe { RoGetActivationFactory(handle(&name), &DispatcherStatics::IID, &mut statics) };
    if hr < 0 || statics.is_null() {
        eprintln!(
            "xaml-probe: RoGetActivationFactory(DispatcherQueueController) hr=0x{:08x}",
            hr as u32
        );
        return Some(hr);
    }
    let statics: DispatcherStatics = unsafe { DispatcherStatics::from_raw(statics) };
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

// `Panel.Children` is a `UIElementCollection`, which implements
// `Windows.Foundation.Collections.IVector<UIElement>`. windows-bindgen projects every
// `IVector<T>` through the `windows-collections` crate, which the probe does not depend
// on, so the two accessors the measurement needs are declared here.
//
// The vtables start with `IInspectable_Vtbl`, which is six slots: declaring a WinRT
// interface's base as `IUnknown` puts the first method where `GetIids` sits, and that
// member answers `S_OK` while writing a pointer that is not an object.
#[repr(transparent)]
#[derive(Clone)]
pub(crate) struct IPanelChildren(IUnknown);

#[repr(C)]
pub(crate) struct IPanelChildrenVtbl {
    base__: windows_core::IInspectable_Vtbl,
    children: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> windows_core::HRESULT,
    put_background: unsafe extern "system" fn(*mut c_void, *mut c_void) -> windows_core::HRESULT,
}

unsafe impl Interface for IPanelChildren {
    type Vtable = IPanelChildrenVtbl;
    const IID: windows_core::GUID =
        windows_core::GUID::from_u128(0x27a1_b418_56f3_525e_b883_cefe_d905_eed3);
}

impl IPanelChildren {
    pub(crate) fn set_background(&self, brush: &IUnknown) -> windows_core::HRESULT {
        // Safety: both pointers are live for the call.
        unsafe {
            (Interface::vtable(self).put_background)(
                Interface::as_raw(self),
                Interface::as_raw(brush),
            )
        }
    }

    fn children(&self) -> windows_core::Result<IUnknown> {
        let mut out: *mut c_void = ptr::null_mut();
        // Safety: the object is a live `Panel` and `out` is writable.
        let hr = unsafe { (Interface::vtable(self).children)(Interface::as_raw(self), &mut out) };
        check("IPanel.Children", hr);
        if out.is_null() {
            fail("IPanel.Children result", E_POINTER);
        }
        Ok(unsafe { IUnknown::from_raw(out) })
    }
}

/// A failed step names itself and its HRESULT; there is no JSON line in that case.
pub(crate) fn fail(step: &str, hr: i32) -> ! {
    eprintln!("xaml-probe: {step} hr=0x{:08x}", hr as u32);
    std::process::exit(1);
}

pub(crate) fn check(step: &str, hr: windows_core::HRESULT) {
    if hr.0 < 0 {
        fail(step, hr.0);
    }
}

pub(crate) fn cast<T: Interface>(object: &impl Interface, step: &str) -> T {
    object
        .cast::<T>()
        .unwrap_or_else(|error| fail(step, error.code().0))
}

/// Ask the engine's own `DllGetActivationFactory` for one class.
pub(crate) fn activation_factory(get: GetActivationFactory, class: &str) -> IUnknown {
    let name = HSTRING::from(class);
    let mut raw: *mut c_void = ptr::null_mut();
    let hr = unsafe { get(handle(&name), &mut raw) };
    let step = format!("DllGetActivationFactory({class})");
    check(&step, windows_core::HRESULT(hr));
    if raw.is_null() {
        fail(&step, E_POINTER);
    }
    unsafe { IUnknown::from_raw(raw) }
}

/// Call a composable factory: `CreateInstance(outer, inner, result)`.
fn create_instance(step: &str, factory: *mut c_void, function: CreateInstance) -> IUnknown {
    let mut inner: *mut c_void = ptr::null_mut();
    let mut out: *mut c_void = ptr::null_mut();
    let hr = unsafe { function(factory, ptr::null_mut(), &mut inner, &mut out) };
    check(step, hr);
    // A null outer leaves `inner` null. A non-null one would be ours to release, but it
    // is not the object being measured; the probe leaks it for the one call it makes.
    if out.is_null() {
        fail(step, E_POINTER);
    }
    unsafe { IUnknown::from_raw(out) }
}

/// Activate a class that has a default constructor, such as `Border`.
fn activate(get: GetActivationFactory, class: &str) -> IUnknown {
    let factory = activation_factory(get, class);
    let activation: IActivationFactory = cast(&factory, "QueryInterface(IActivationFactory)");
    let step = format!("{class}.ActivateInstance");
    activation
        .activate()
        .unwrap_or_else(|error| fail(&step, error.code().0))
}

/// The one layout question: a vertical `StackPanel`, `Spacing = 4`, three 10 px high
/// `Border` children. The panel is expected to want 38 px; the numbers printed are the
/// ones the engine returns, not that expectation.
/// Say which step the measurement reached when `OPENCOMP_TRACE` is set.
///
/// A call that faults prints nothing of its own; the last mark names it.
pub(crate) fn mark(step: &str) {
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!("xaml-probe: measure {step}");
    }
}

fn measure(get: GetActivationFactory, app_factory: &IApplicationFactory) {
    mark("application");
    // The application WinUI's own entry point creates inside this callback. The factory
    // came from the DLL's `DllGetActivationFactory`, so no combase registration is needed.
    let application = create_instance(
        "IApplicationFactory.CreateInstance",
        Interface::as_raw(app_factory),
        Interface::vtable(app_factory).CreateInstance,
    );

    mark("stack panel factory");
    let stack_factory = activation_factory(get, "Microsoft.UI.Xaml.Controls.StackPanel");
    let stack_factory: IStackPanelFactory =
        cast(&stack_factory, "QueryInterface(IStackPanelFactory)");
    mark("stack panel instance");
    let stack = create_instance(
        "IStackPanelFactory.CreateInstance",
        Interface::as_raw(&stack_factory),
        Interface::vtable(&stack_factory).CreateInstance,
    );

    // `StackPanel`'s default orientation is vertical, which is the case being measured.
    let stack_panel: IStackPanel = cast(&stack, "QueryInterface(IStackPanel)");
    stack_panel
        .SetSpacing(4.0)
        .unwrap_or_else(|error| fail("IStackPanel.Spacing", error.code().0));

    mark("panel children");
    let panel: IPanelChildren = cast(&stack, "QueryInterface(IPanel)");
    let collection = panel
        .children()
        .unwrap_or_else(|error| fail("IPanel.Children", error.code().0));
    let collection: UIElementCollection = cast(&collection, "QueryInterface(UIElementCollection)");
    let children: windows_collections::IVector<UIElement> =
        cast(&collection, "QueryInterface(IVector<UIElement>)");

    mark("borders");
    let mut first: Option<IUnknown> = None;
    for index in 0..3 {
        mark("border activate");
        let border = activate(get, "Microsoft.UI.Xaml.Controls.Border");
        mark("border height");
        let framework: IFrameworkElement = cast(&border, "QueryInterface(IFrameworkElement)");
        framework
            .SetHeight(10.0)
            .unwrap_or_else(|error| fail("IFrameworkElement.Height", error.code().0));
        mark("border append");
        let element: UIElement = cast(&border, "QueryInterface(UIElement)");
        children
            .Append(&element)
            .unwrap_or_else(|error| fail("IVector<UIElement>.Append", error.code().0));
        if index == 0 {
            first = Some(border);
        }
    }

    mark("measure");
    let ui: IUIElement = cast(&stack, "QueryInterface(IUIElement)");
    ui.Measure(Size {
        width: f32::INFINITY,
        height: f32::INFINITY,
    })
    .unwrap_or_else(|error| fail("IUIElement.Measure", error.code().0));
    let desired = ui
        .DesiredSize()
        .unwrap_or_else(|error| fail("IUIElement.DesiredSize", error.code().0));
    ui.Arrange(Rect {
        x: 0.0,
        y: 0.0,
        width: 1000.0,
        height: 1000.0,
    })
    .unwrap_or_else(|error| fail("IUIElement.Arrange", error.code().0));

    let Some(first) = first else {
        fail("first Border", E_POINTER);
    };
    let child: IUIElement = cast(&first, "QueryInterface(first child IUIElement)");
    let child_desired = child
        .DesiredSize()
        .unwrap_or_else(|error| fail("first child DesiredSize", error.code().0));

    // One line, the shape the Gate B job consumes.
    println!(
        "{{\"case\":\"stackpanel-spacing4\",\"width\":{},\"height\":{},\"child0_height\":{}}}",
        desired.width, desired.height, child_desired.height
    );
    // The callback exits the process, which does not flush Rust's buffered stdout.
    std::io::stdout().flush().ok();
    // Releasing the application would run the engine's teardown for no reason.
    std::mem::forget(application);
}

/// `MddBootstrapInitialize(majorMinor, versionTag, minVersion)` from the SDK's bootstrapper,
/// which lives beside the XAML DLL.
fn bootstrap() -> i32 {
    let path = std::env::var_os("XAML_DLL")
        .map(std::path::PathBuf::from)
        .map(|path| path.with_file_name("Microsoft.WindowsAppRuntime.Bootstrap.dll"));
    let Some(module) = path.and_then(|path| crate::windows::load(&path)) else {
        return 0x8007_007E_u32 as i32; // ERROR_MOD_NOT_FOUND as an HRESULT
    };
    let Some(entry) = crate::windows::export(module, "MddBootstrapInitialize") else {
        return 0x8000_4002_u32 as i32; // E_NOINTERFACE
    };
    let function: unsafe extern "system" fn(u32, *const u16, u64) -> i32 =
        unsafe { std::mem::transmute(entry) };
    // 2.7 as `major << 16 | minor`, a null version tag, and the lowest version.
    unsafe { function(0x0002_0007, std::ptr::null(), 0) }
}

pub(crate) fn run(module: HMODULE) -> i32 {
    // Safety: the apartment is process state and this is the first COM call the probe makes.
    let hr = unsafe { RoInitialize(0) };
    eprintln!("xaml-probe: RoInitialize(STA) hr=0x{:08x}", hr as u32);
    let Some(get_factory) = export(module, "DllGetActivationFactory") else {
        eprintln!("xaml-probe: DllGetActivationFactory missing");
        return 1;
    };
    // `HRESULT DllGetActivationFactory(HSTRING, IActivationFactory **)`.
    let get_factory: GetActivationFactory = unsafe { std::mem::transmute(get_factory) };
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
    // An unpackaged process has no package identity, and `Application.Start` answers
    // `E_NOTIMPL` without the Windows App SDK runtime behind it. The bootstrapper is the
    // documented answer; it aborts on Wine 9 because its error path calls a `combase`
    // stub, so it stays opt-in until the Wine job runs a version that has that function.
    if std::env::var_os("GATE_B_BOOTSTRAP").is_some() {
        eprintln!(
            "xaml-probe: MddBootstrapInitialize hr=0x{:08x}",
            bootstrap()
        );
    }
    // Wine carries the process-policy questions XAML asks as stubs that abort on call;
    // the engine's own import slots get our answers before it runs.
    for (library, name) in crate::shim::install(module) {
        eprintln!("xaml-probe: answered {name} for {library}");
    }
    // The input core is delay-loaded by the UDK layer, so that module is where the
    // resolver has to answer instead of Wine's stub.
    // The input core is Wine's stub. Our session object answers it; `OPENCOMP_COREUI=xp`
    // asks the App SDK's own CoreMessaging instead, which runs once its ntdll completion
    // packet calls are answered and then stops in `NtAlpcConnectPort`, which Wine only
    // stubs -- so it is not the default.
    let xp = std::env::var_os("OPENCOMP_COREUI").is_some_and(|value| value == "xp");
    if xp {
        if let Some(messaging) = crate::windows::load(std::path::Path::new(MESSAGING)) {
            for (library, name) in crate::shim::install_late(messaging) {
                eprintln!("xaml-probe: answered {name} for {library} (messaging)");
            }
            if crate::shim::answer_core_ui_with_messaging(messaging) {
                eprintln!("xaml-probe: CoreUICreate from {MESSAGING}");
            }
        }
    } else if std::env::var_os("OPENCOMP_COREUI").is_some_and(|value| value == "proxy") {
        // Measurement on the platform where the real object exists: the same call, the
        // same session, with every method the engine uses printed.
        if crate::proxy::install_and_answer() {
            eprintln!("xaml-probe: CoreUICreate from a proxy over the system session");
        }
    } else {
        let ours = crate::windows::load(std::path::Path::new(OWN_MESSAGING));
        if let Some(create) = ours.and_then(|ours| crate::windows::export(ours, "CoreUICreate")) {
            crate::shim::answer_core_ui(create as *mut std::ffi::c_void);
            eprintln!("xaml-probe: CoreUICreate from {OWN_MESSAGING}");
        }
    }
    if let Some(udk) = crate::windows::load(std::path::Path::new(UDK)) {
        for (library, name) in crate::shim::install(udk) {
            eprintln!("xaml-probe: answered {name} for {library} (udk)");
        }
    }
    if let Some(hr) = create_dispatcher() {
        eprintln!(
            "xaml-probe: DispatcherQueueController.CreateOnCurrentThread hr=0x{:08x}",
            hr as u32
        );
    }
    let factory: IUnknown = unsafe { IUnknown::from_raw(raw) };
    let Ok(statics) = factory.cast::<IApplicationStatics>() else {
        eprintln!("xaml-probe: no IApplicationStatics on the factory");
        return 1;
    };
    let app_factory: IApplicationFactory = cast(&factory, "QueryInterface(IApplicationFactory)");

    let callback = ApplicationInitializationCallback::new(move |_params| {
        // The engine called us on its own thread with a core behind it. Build the case,
        // print its numbers, and leave the message loop: there is nothing after this.
        measure(get_factory, &app_factory);
        if std::env::var_os("OPENCOMP_XAML_PROBE_WINDOW").is_some() {
            crate::window::show(get_factory);
        }
        // `process::exit` runs the C runtime's teardown, which does not return once the
        // engine owns threads, and `ExitProcess` waits for them. The probe has its number
        // and nothing left to say, so the process is ended rather than shut down.
        unsafe { TerminateProcess(GetCurrentProcess(), 0) };
    });
    eprintln!("xaml-probe: calling Application.Start");

    // The generated vtable names the slot and the generated delegate carries the ABI;
    // `--minimal` leaves out only the method wrapper, which is this one line.
    let hr = unsafe {
        (Interface::vtable(&statics).Start)(
            Interface::as_raw(&statics),
            Interface::as_raw(&callback),
        )
    };
    // A start that returns without calling back never built a core.
    eprintln!(
        "xaml-probe: Application.Start returned hr=0x{:08x}",
        hr.0 as u32
    );
    1
}
