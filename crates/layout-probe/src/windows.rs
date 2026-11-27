//! The Windows half: load the DLL, create one XAML context, ask it one layout question.
//!
//! The engine is not the workspace's `openxaml`; it is the closed `Microsoft.ui.xaml.dll`.
//! Only the ABI slots this probe calls are declared here. Slot numbers are method indices
//! from the metadata, plus the three `IInspectable` slots (`QueryInterface`, `AddRef`,
//! `Release`) that every WinRT interface starts with.

use std::ffi::OsStr;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr;

use windows_core::{GUID, HSTRING};
use windows_sys::Win32::Foundation::{GetLastError, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

use crate::abi::*;
use crate::callback::CallbackHandle;

type Export = unsafe extern "system" fn() -> isize;

fn handle(name: &HSTRING) -> Raw {
    unsafe { std::mem::transmute_copy(name) }
}

fn load(path: &Path) -> Result<HMODULE, Failure> {
    let wide: Vec<u16> = OsStr::new(path).encode_wide().chain(Some(0)).collect();
    let module = unsafe { LoadLibraryW(wide.as_ptr()) };
    if module.is_null() {
        let error = unsafe { GetLastError() };
        eprintln!(
            "layout-probe: LoadLibraryW {} failed: GetLastError=0x{error:08x}",
            path.display()
        );
        return Err(Failure::new("LoadLibraryW", E_FAIL));
    }
    eprintln!("layout-probe: LoadLibraryW {} ok", path.display());
    Ok(module)
}

fn export(module: HMODULE, name: &str) -> Option<Export> {
    let mut bytes: Vec<u8> = name.bytes().collect();
    bytes.push(0);
    unsafe { GetProcAddress(module, bytes.as_ptr()) }
}

fn activation_factory(module: HMODULE, class: &str) -> Result<Raw, Failure> {
    let function = export(module, "DllGetActivationFactory")
        .ok_or(Failure::new("DllGetActivationFactory export", E_FAIL))?;
    let function: ActivationFactory = unsafe { std::mem::transmute(function) };
    let name = HSTRING::from(class);
    let mut out: Raw = ptr::null_mut();
    let hr = unsafe { function(handle(&name), &mut out) };
    eprintln!(
        "layout-probe: DllGetActivationFactory({class}) hr=0x{:08x}",
        hr as u32
    );
    if hr < 0 {
        return Err(Failure::new("DllGetActivationFactory", hr));
    }
    if out.is_null() {
        return Err(Failure::new("DllGetActivationFactory result", E_POINTER));
    }
    Ok(out)
}

/// Call a composable factory. `StackPanel` and `Application` have no default constructor:
/// their factory is `CreateInstance(outer, inner, result)`, not `IActivationFactory`.
fn create_instance(factory: Raw, iid: GUID, step: &'static str) -> Result<Raw, Failure> {
    let interface = unsafe { query(factory, iid) }.map_err(|hr| Failure::new(step, hr))?;
    let mut inner: Raw = ptr::null_mut();
    let mut out: Raw = ptr::null_mut();
    let function: CreateInstance = unsafe { slot(interface, 3) };
    let hr = unsafe { function(interface, ptr::null_mut(), &mut inner, &mut out) };
    unsafe { release(interface) };
    eprintln!("layout-probe: {step} hr=0x{:08x}", hr as u32);
    // A null outer should leave `inner` null. If the engine returns one, it is leaked:
    // the probe is short lived and this is not the object we measure.
    if hr < 0 {
        return Err(Failure::new(step, hr));
    }
    if out.is_null() {
        return Err(Failure::new(step, E_POINTER));
    }
    Ok(out)
}

fn activate(factory: Raw, step: &'static str) -> Result<Raw, Failure> {
    let interface =
        unsafe { query(factory, IID_ACTIVATION_FACTORY) }.map_err(|hr| Failure::new(step, hr))?;
    let mut out: Raw = ptr::null_mut();
    let function: ActivateInstance = unsafe { slot(interface, 3) };
    let hr = unsafe { function(interface, &mut out) };
    unsafe { release(interface) };
    eprintln!("layout-probe: {step} hr=0x{:08x}", hr as u32);
    if hr < 0 {
        return Err(Failure::new(step, hr));
    }
    if out.is_null() {
        return Err(Failure::new(step, E_POINTER));
    }
    Ok(out)
}

/// Ask the dispatcher XAML installed for this thread. This runs inside the
/// `Application.Start` callback: Start creates the thread's DispatcherQueue before it
/// calls us, which is also why the probe must not create one with
/// `CreateOnCurrentThread` first (that method errors when one is already associated).
fn current_dispatcher(module: HMODULE) -> Result<Raw, Failure> {
    let factory = activation_factory(module, "Microsoft.UI.Dispatching.DispatcherQueue")?;
    let statics = unsafe { query(factory, IID_DISPATCHER_QUEUE_STATICS) }
        .map_err(|hr| Failure::new("QueryInterface(IDispatcherQueueStatics)", hr))?;
    let mut queue: Raw = ptr::null_mut();
    let function: GetObject = unsafe { slot(statics, 3) };
    let hr = unsafe { function(statics, &mut queue) };
    unsafe { release(statics) };
    eprintln!(
        "layout-probe: DispatcherQueue.GetForCurrentThread hr=0x{:08x}",
        hr as u32
    );
    if hr < 0 {
        return Err(Failure::new("DispatcherQueue.GetForCurrentThread", hr));
    }
    if queue.is_null() {
        return Err(Failure::new(
            "DispatcherQueue.GetForCurrentThread result",
            E_POINTER,
        ));
    }
    Ok(queue)
}

fn create_stack(factory: Raw) -> Result<Raw, Failure> {
    match create_instance(
        factory,
        IID_STACK_PANEL_FACTORY,
        "StackPanel.CreateInstance",
    ) {
        Ok(stack) => Ok(stack),
        Err(_) => {
            eprintln!("layout-probe: StackPanel.CreateInstance failed; trying IActivationFactory");
            activate(factory, "StackPanel.ActivateInstance")
        }
    }
}

fn case(module: HMODULE, ui_module: HMODULE, app_factory: Raw) -> Result<Reading, Failure> {
    if !ui_module.is_null() {
        match current_dispatcher(ui_module) {
            Ok(queue) => {
                let _queue = queue;
                eprintln!("layout-probe: measured on the XAML DispatcherQueue");
            }
            Err(failure) => failure.report(),
        }
    }

    // A base Application is composable, not activatable. It is best effort: if the
    // engine refuses a base instance, the measurement below still reports its own
    // result instead of hiding it behind this setup step.
    match create_instance(
        app_factory,
        IID_APPLICATION_FACTORY,
        "Application.CreateInstance",
    ) {
        Ok(app) => {
            let _app = app;
        }
        Err(failure) => eprintln!(
            "layout-probe: no base Application (hr=0x{:08x}); continuing",
            failure.hr as u32
        ),
    }

    let stack_factory = activation_factory(module, "Microsoft.UI.Xaml.Controls.StackPanel")?;
    let stack = create_stack(stack_factory)?;

    let panel = unsafe { query(stack, IID_PANEL) }
        .map_err(|hr| Failure::new("QueryInterface(IPanel)", hr))?;
    let mut collection: Raw = ptr::null_mut();
    let children: GetObject = unsafe { slot(panel, 3) };
    let hr = unsafe { children(panel, &mut collection) };
    eprintln!("layout-probe: IPanel.Children hr=0x{:08x}", hr as u32);
    if hr < 0 {
        return Err(Failure::new("IPanel.Children", hr));
    }
    if collection.is_null() {
        return Err(Failure::new("IPanel.Children result", E_POINTER));
    }
    let vector = unsafe { query(collection, IID_VECTOR_UIELEMENT) }
        .map_err(|hr| Failure::new("QueryInterface(IVector<UIElement>)", hr))?;

    let stack_panel = unsafe { query(stack, IID_STACK_PANEL) }
        .map_err(|hr| Failure::new("QueryInterface(IStackPanel)", hr))?;
    let put_spacing: PutF64 = unsafe { slot(stack_panel, 3 + 15) };
    let hr = unsafe { put_spacing(stack_panel, 4.0) };
    eprintln!("layout-probe: IStackPanel.Spacing=4 hr=0x{:08x}", hr as u32);
    if hr < 0 {
        return Err(Failure::new("IStackPanel.Spacing", hr));
    }

    let border_factory = activation_factory(module, "Microsoft.UI.Xaml.Controls.Border")?;
    let mut first_child: Raw = ptr::null_mut();
    for index in 0..3 {
        let border = activate(border_factory, "Border.ActivateInstance")?;
        if index == 0 {
            first_child = border;
        }
        let element = unsafe { query(border, IID_FRAMEWORK_ELEMENT) }
            .map_err(|hr| Failure::new("QueryInterface(IFrameworkElement)", hr))?;
        let put_height: PutF64 = unsafe { slot(element, 3 + 12) };
        let hr = unsafe { put_height(element, 10.0) };
        eprintln!(
            "layout-probe: Border[{index}].Height=10 hr=0x{:08x}",
            hr as u32
        );
        if hr < 0 {
            return Err(Failure::new("IFrameworkElement.Height", hr));
        }
        let append: Append = unsafe { slot(vector, 3 + 7) };
        let hr = unsafe { append(vector, border) };
        eprintln!(
            "layout-probe: Children.Append({index}) hr=0x{:08x}",
            hr as u32
        );
        if hr < 0 {
            return Err(Failure::new("IVector<UIElement>.Append", hr));
        }
    }

    let ui = unsafe { query(stack, IID_UIELEMENT) }
        .map_err(|hr| Failure::new("QueryInterface(IUIElement)", hr))?;
    let measure: Measure = unsafe { slot(ui, 3 + 204) };
    let hr = unsafe {
        measure(
            ui,
            Size {
                width: f32::INFINITY,
                height: f32::INFINITY,
            },
        )
    };
    eprintln!(
        "layout-probe: IUIElement.Measure(inf,inf) hr=0x{:08x}",
        hr as u32
    );
    if hr < 0 {
        return Err(Failure::new("IUIElement.Measure", hr));
    }

    let mut desired = Size::default();
    let get_desired: GetSize = unsafe { slot(ui, 3) };
    let hr = unsafe { get_desired(ui, &mut desired) };
    eprintln!(
        "layout-probe: IUIElement.DesiredSize hr=0x{:08x} ({} x {})",
        hr as u32, desired.width, desired.height
    );
    if hr < 0 {
        return Err(Failure::new("IUIElement.DesiredSize", hr));
    }

    let arrange: Arrange = unsafe { slot(ui, 3 + 205) };
    let hr = unsafe {
        arrange(
            ui,
            Rect {
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 1000.0,
            },
        )
    };
    eprintln!(
        "layout-probe: IUIElement.Arrange(0,0,1000,1000) hr=0x{:08x}",
        hr as u32
    );
    if hr < 0 {
        return Err(Failure::new("IUIElement.Arrange", hr));
    }

    let child_ui = unsafe { query(first_child, IID_UIELEMENT) }
        .map_err(|hr| Failure::new("QueryInterface(first child IUIElement)", hr))?;
    let mut child_desired = Size::default();
    let get_child_desired: GetSize = unsafe { slot(child_ui, 3) };
    let hr = unsafe { get_child_desired(child_ui, &mut child_desired) };
    eprintln!(
        "layout-probe: child[0].DesiredSize hr=0x{:08x} ({} x {})",
        hr as u32, child_desired.width, child_desired.height
    );
    if hr < 0 {
        return Err(Failure::new("first child DesiredSize", hr));
    }

    Ok(Reading {
        width: desired.width,
        height: desired.height,
        child0_height: child_desired.height,
    })
}

/// Initialize the Windows App SDK runtime, if the bootstrapper is beside the XAML DLL.
///
/// `PACKAGE_VERSION` is a union of a `u64` and four `u16`s, and x64 passes it in one
/// register, so a `u64` keeps the ABI without the struct.
fn bootstrap_hr(dir: &Path) -> Option<i32> {
    let path = dir.join("Microsoft.WindowsAppRuntime.Bootstrap.dll");
    let module = load(&path).ok()?;
    let function = export(module, "MddBootstrapInitialize")?;
    let function: unsafe extern "system" fn(u32, *const u16, u64) -> i32 =
        unsafe { std::mem::transmute(function) };
    // 2.7 as `major << 16 | minor`, a null version tag, and the lowest version.
    const MAJOR_MINOR: u32 = 0x0002_0007;
    Some(unsafe { function(MAJOR_MINOR, std::ptr::null(), 0) })
}

fn run_case(module: usize, ui_module: usize, app_factory: Raw, out: Option<PathBuf>) {
    let reading = case(module as HMODULE, ui_module as HMODULE, app_factory);
    match reading {
        Ok(reading) => {
            let line = format!(
                "{{\"case\":\"stackpanel-spacing4\",\"width\":{},\"height\":{},\"child0_height\":{}}}",
                reading.width, reading.height, reading.child0_height
            );
            if let Err(error) = emit(&line, out.as_deref()) {
                eprintln!("layout-probe: writing the result failed: {error}");
                std::process::exit(1);
            }
            std::process::exit(0);
        }
        Err(failure) => {
            failure.report();
            std::process::exit(1);
        }
    }
}

fn emit(line: &str, out: Option<&Path>) -> std::io::Result<()> {
    match out {
        Some(path) => {
            let mut file = std::fs::File::create(path)?;
            file.write_all(line.as_bytes())?;
            file.write_all(b"\n")?;
            file.sync_all()
        }
        None => {
            let mut stdout = std::io::stdout().lock();
            stdout.write_all(line.as_bytes())?;
            stdout.write_all(b"\n")?;
            stdout.flush()
        }
    }
}

pub fn run(out: Option<PathBuf>) -> i32 {
    let Some(path) = std::env::var_os("XAML_DLL") else {
        eprintln!("layout-probe: XAML_DLL is not set");
        return 2;
    };
    let path = PathBuf::from(path);

    let hr = init_apartment();
    eprintln!("layout-probe: RoInitialize(STA) hr=0x{:08x}", hr as u32);
    if hr < 0 {
        return 1;
    }

    let module = match load(&path) {
        Ok(module) => module,
        Err(failure) => {
            failure.report();
            return 2;
        }
    };

    // An unpackaged process has no package identity, and XAML's core refuses to
    // initialize without the Windows App SDK runtime behind it. The bootstrapper is the
    // documented answer: `MddBootstrapInitialize(majorMinor, versionTag, minVersion)`.
    // Its own failure is reported and not treated as fatal, because it is a separate
    // question from whether XAML then works.
    if let Some(bootstrap) = bootstrap_hr(path.parent().unwrap_or(Path::new("."))) {
        eprintln!(
            "layout-probe: MddBootstrapInitialize hr=0x{:08x}",
            bootstrap as u32
        );
    }

    match export(module, "XamlCheckProcessRequirements") {
        Some(function) => {
            let function: unsafe extern "system" fn() -> i32 =
                unsafe { std::mem::transmute(function) };
            let hr = unsafe { function() };
            eprintln!(
                "layout-probe: XamlCheckProcessRequirements hr=0x{:08x}",
                hr as u32
            );
        }
        None => eprintln!("layout-probe: XamlCheckProcessRequirements missing"),
    }

    // Microsoft.UI.dll carries the dispatcher statics. It is next to the XAML DLL in
    // the Windows App SDK layout. Failure is reported, not fatal: the callback below
    // only uses it to confirm which dispatcher Application.Start installed.
    let ui_path = path.with_file_name("Microsoft.UI.dll");
    let ui_module = match load(&ui_path) {
        Ok(ui) => ui,
        Err(failure) => {
            failure.report();
            ptr::null_mut()
        }
    };

    let app_factory = match activation_factory(module, "Microsoft.UI.Xaml.Application") {
        Ok(factory) => factory,
        Err(failure) => {
            failure.report();
            return 1;
        }
    };
    let statics = match unsafe { query(app_factory, IID_APPLICATION_STATICS) } {
        Ok(statics) => statics,
        Err(hr) => {
            Failure::new("QueryInterface(IApplicationStatics)", hr).report();
            return 1;
        }
    };

    // The callback runs on the dispatcher Application.Start installs for this thread.
    // `current_dispatcher` confirms it before the first Measure.
    let module_raw = module as usize;
    let ui_raw = ui_module as usize;
    let callback = CallbackHandle::new(move || run_case(module_raw, ui_raw, app_factory, out));
    let start: Start = unsafe { slot(statics, 3 + 1) };
    let hr = unsafe { start(statics, callback.raw()) };
    eprintln!(
        "layout-probe: Application.Start returned hr=0x{:08x}",
        hr as u32
    );
    unsafe { release(statics) };
    if hr < 0 {
        Failure::new("Application.Start", hr).report();
    }
    1
}
