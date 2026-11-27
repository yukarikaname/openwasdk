//! The Windows half of the probe: load the DLL, walk its exports, activate the classes.
//!
//! Activation goes through the DLL's own `DllGetActivationFactory`, not through the WinRT
//! runtime, because the point is to test this DLL rather than the registration of the machine.
//! Classes with a default constructor are activated and their default interface is queried;
//! classes that only have statics hand back their statics interface, which is what a factory
//! for those classes implements.

use std::ffi::{OsStr, c_void};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

use windows_core::{HSTRING, IUnknown, Interface};
use windows_sys::Win32::Foundation::{GetLastError, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

use crate::xaml_bindings::Microsoft::UI::Content::IContentIslandStatics;
use crate::xaml_bindings::Microsoft::UI::Xaml::Controls::{
    IButtonStatics, IFrameStatics, IGridStatics, IStackPanelStatics, ITextBlock,
};
use crate::xaml_bindings::Microsoft::UI::Xaml::Media::ISolidColorBrush;
use crate::xaml_bindings::Microsoft::UI::Xaml::{IApplicationStatics, IWindowStatics};

const E_FAIL: i32 = 0x8000_4005u32 as i32;
const E_NOINTERFACE: i32 = 0x8000_4002u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;

/// The exports besides `XamlCheckProcessRequirements` that the DLL is expected to carry.
const OTHER_EXPORTS: &[&str] = &[
    "DllGetActivationFactory",
    "DllCanUnloadNow",
    "DllMain",
    "OverrideXamlMetadataProvider",
    "OverrideXamlResourcePropertyBag",
    "GetErrorContextIndex",
    "GetHeapHandle",
];

/// A default interface to query, with one call so the instance answers through its vtable.
type Check = fn(&IUnknown) -> windows_core::Result<()>;

struct Entry {
    /// The runtime class name as `DllGetActivationFactory` expects it.
    name: &'static str,
    /// The default interface, when the metadata gives the class a default constructor.
    instance: Option<Check>,
    /// The statics interface, when the class has no default constructor.
    statics: Option<Check>,
}

/// The classes Gate B asks about. `Microsoft.UI.Xaml.Application` appears twice in the task
/// list; a class is reported once, so the duplicate is dropped here.
const CLASSES: &[Entry] = &[
    Entry {
        name: "Microsoft.UI.Xaml.Application",
        instance: None,
        statics: Some(check_application_statics),
    },
    Entry {
        name: "Microsoft.UI.Xaml.Window",
        instance: None,
        statics: Some(check_window_statics),
    },
    Entry {
        name: "Microsoft.UI.Xaml.Controls.TextBlock",
        instance: Some(check_text_block),
        statics: None,
    },
    Entry {
        name: "Microsoft.UI.Xaml.Controls.Button",
        instance: None,
        statics: Some(check_button_statics),
    },
    Entry {
        name: "Microsoft.UI.Xaml.Controls.Grid",
        instance: None,
        statics: Some(check_grid_statics),
    },
    Entry {
        name: "Microsoft.UI.Xaml.Controls.StackPanel",
        instance: None,
        statics: Some(check_stack_panel_statics),
    },
    Entry {
        name: "Microsoft.UI.Xaml.Media.SolidColorBrush",
        instance: Some(check_solid_color_brush),
        statics: None,
    },
    Entry {
        name: "Microsoft.UI.Xaml.Controls.Frame",
        instance: None,
        statics: Some(check_frame_statics),
    },
    Entry {
        name: "Microsoft.UI.Content.ContentIsland",
        instance: None,
        statics: Some(check_content_island_statics),
    },
];

// `IActivationFactory` is not a runtime class, so it is not in the metadata. The vtable is
// written out rather than declared with `interface_decl!`: that macro's `IInspectable` base
// does not compile against this windows-core, and an `IUnknown` base puts `ActivateInstance`
// where `GetIids` sits -- which answers `S_OK` and writes an empty array, so every "the class
// activated" result looked successful and meant nothing.
#[repr(transparent)]
#[derive(Clone)]
pub(crate) struct IActivationFactory(windows_core::IUnknown);

#[repr(C)]
pub(crate) struct IActivationFactoryVtbl {
    base__: windows_core::IInspectable_Vtbl,
    activate_instance:
        unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> windows_core::HRESULT,
}

// Safety: the layout is `IInspectable` followed by `ActivateInstance`, which is the ABI.
unsafe impl windows_core::Interface for IActivationFactory {
    type Vtable = IActivationFactoryVtbl;
    const IID: windows_core::GUID =
        windows_core::GUID::from_u128(0x0000_0035_0000_0000_c000_0000_0000_0046);
}

impl IActivationFactory {
    pub(crate) fn activate(&self) -> windows_core::Result<windows_core::IUnknown> {
        let mut raw: *mut c_void = std::ptr::null_mut();
        // Safety: the object is a live activation factory and `raw` is writable.
        let hr = unsafe {
            (windows_core::Interface::vtable(self).activate_instance)(
                windows_core::Interface::as_raw(self),
                &mut raw,
            )
        };
        if hr.0 < 0 {
            return Err(windows_core::Error::from_hresult(hr));
        }
        if raw.is_null() {
            return Err(windows_core::Error::from_hresult(windows_core::HRESULT(
                0x8000_4003u32 as i32,
            )));
        }
        Ok(unsafe { windows_core::IUnknown::from_raw(raw) })
    }
}

struct Outcome {
    factory: usize,
    instance: usize,
    hr: i32,
}

pub fn run() -> i32 {
    let json = std::env::var("OPENCOMP_XAML_PROBE_JSON").is_ok_and(|value| value == "1");
    let Some(path) = std::env::var_os("XAML_DLL") else {
        eprintln!("xaml-probe: XAML_DLL is not set");
        return 2;
    };

    let module = match load(Path::new(&path)) {
        Some(module) => module,
        None => return 2,
    };
    report_dll(json, &path.to_string_lossy());

    let check = export(module, "XamlCheckProcessRequirements");
    let check_hr = check.map(|function| {
        // The export is `HRESULT XamlCheckProcessRequirements()`.
        let function: unsafe extern "system" fn() -> i32 = unsafe { std::mem::transmute(function) };
        unsafe { function() }
    });
    report_check(json, check_hr);

    if std::env::var_os("OPENCOMP_XAML_PROBE_START").is_some() {
        return crate::start::run(module);
    }

    for name in OTHER_EXPORTS {
        report_export(json, name, export(module, name).is_some());
    }

    let get_factory = export(module, "DllGetActivationFactory").map(|function| {
        // `HRESULT DllGetActivationFactory(HSTRING, IActivationFactory**)`.
        let function: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32 =
            unsafe { std::mem::transmute(function) };
        function
    });

    // A crash inside the engine takes the rest of the report with it, so the set can be
    // narrowed from outside: `OPENCOMP_XAML_PROBE_ONLY=Controls.` runs the classes whose
    // name contains that text. Without it every class is probed.
    let only = std::env::var("OPENCOMP_XAML_PROBE_ONLY").ok();
    let mut any = false;
    for entry in CLASSES {
        if let Some(needle) = &only
            && !entry.name.contains(needle.as_str())
        {
            continue;
        }
        let outcome = match get_factory {
            Some(get) => probe(entry, get),
            None => Outcome {
                factory: 0,
                instance: 0,
                hr: E_FAIL,
            },
        };
        any |= outcome.hr >= 0;
        report_class(json, entry.name, &outcome);
    }

    if any { 0 } else { 1 }
}

/// Load the DLL by its full path. The process keeps the module for its lifetime; the probe is
/// short lived and freeing it would only race the factories that are still referenced.
pub(crate) fn load(path: &Path) -> Option<HMODULE> {
    let wide: Vec<u16> = OsStr::new(path).encode_wide().chain(Some(0)).collect();
    let module = unsafe { LoadLibraryW(wide.as_ptr()) };
    if module.is_null() {
        let error = unsafe { GetLastError() };
        eprintln!(
            "xaml-probe: LoadLibraryW {} failed: GetLastError=0x{error:08x}",
            path.display()
        );
        return None;
    }
    Some(module)
}

pub(crate) fn export(module: HMODULE, name: &str) -> Option<unsafe extern "system" fn() -> isize> {
    let mut bytes: Vec<u8> = name.bytes().collect();
    bytes.push(0);
    // `windows-sys` models a null FARPROC as an absent function.
    unsafe { GetProcAddress(module, bytes.as_ptr()) }
}

/// Ask the DLL for the factory of one class, then either activate it or take its statics.
fn probe(
    entry: &Entry,
    get_factory: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32,
) -> Outcome {
    let name = HSTRING::from(entry.name);
    let mut raw: *mut c_void = ptr::null_mut();
    let hr = unsafe { get_factory(handle(&name), &mut raw) };
    if hr < 0 || raw.is_null() {
        return Outcome {
            factory: raw as usize,
            instance: 0,
            hr,
        };
    }

    let factory_ptr = raw as usize;
    let factory = unsafe { IUnknown::from_raw(raw) };
    match (entry.instance, entry.statics) {
        (Some(check), _) => activate(factory, factory_ptr, check),
        (None, Some(check)) => Outcome {
            factory: factory_ptr,
            instance: 0,
            hr: result_code(check(&factory)),
        },
        (None, None) => Outcome {
            factory: factory_ptr,
            instance: 0,
            hr: E_FAIL,
        },
    }
}

fn activate(factory: IUnknown, factory_ptr: usize, check: Check) -> Outcome {
    let Ok(activation) = factory.cast::<IActivationFactory>() else {
        return Outcome {
            factory: factory_ptr,
            instance: 0,
            hr: E_NOINTERFACE,
        };
    };
    let raw = match activation.activate() {
        Ok(instance) => windows_core::Interface::into_raw(instance),
        Err(error) => {
            return Outcome {
                factory: factory_ptr,
                instance: 0,
                hr: error.code().0,
            };
        }
    };
    if raw.is_null() {
        return Outcome {
            factory: factory_ptr,
            instance: 0,
            hr: E_POINTER,
        };
    }

    let instance_ptr = raw as usize;
    let instance = unsafe { IUnknown::from_raw(raw) };
    Outcome {
        factory: factory_ptr,
        instance: instance_ptr,
        hr: result_code(check(&instance)),
    }
}

/// WinRT passes the HSTRING handle by value, which is the pointer inside the HSTRING.
pub(crate) fn handle(name: &HSTRING) -> *mut c_void {
    unsafe { std::mem::transmute_copy(name) }
}

fn result_code(result: windows_core::Result<()>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(error) => error.code().0,
    }
}

fn check_text_block(object: &IUnknown) -> windows_core::Result<()> {
    // The generated `ITextBlock` keeps no callable methods: its event declarations name a
    // handler box the generator does not qualify across modules, so the interface is only
    // available as a query target. The brush below still calls a real vtable slot.
    object.cast::<ITextBlock>().map(|_| ())
}

fn check_solid_color_brush(object: &IUnknown) -> windows_core::Result<()> {
    let brush: ISolidColorBrush = object.cast()?;
    brush.Color().map(|_| ())
}

fn check_application_statics(object: &IUnknown) -> windows_core::Result<()> {
    object.cast::<IApplicationStatics>().map(|_| ())
}

fn check_window_statics(object: &IUnknown) -> windows_core::Result<()> {
    object.cast::<IWindowStatics>().map(|_| ())
}

fn check_button_statics(object: &IUnknown) -> windows_core::Result<()> {
    object.cast::<IButtonStatics>().map(|_| ())
}

fn check_grid_statics(object: &IUnknown) -> windows_core::Result<()> {
    object.cast::<IGridStatics>().map(|_| ())
}

fn check_stack_panel_statics(object: &IUnknown) -> windows_core::Result<()> {
    object.cast::<IStackPanelStatics>().map(|_| ())
}

fn check_frame_statics(object: &IUnknown) -> windows_core::Result<()> {
    object.cast::<IFrameStatics>().map(|_| ())
}

fn check_content_island_statics(object: &IUnknown) -> windows_core::Result<()> {
    object.cast::<IContentIslandStatics>().map(|_| ())
}

fn report_dll(json: bool, path: &str) {
    if json {
        println!(
            "{{\"kind\":\"dll\",\"path\":\"{}\",\"ok\":true}}",
            escaped(path)
        );
    } else {
        println!("xaml-probe: loaded {path}");
    }
}

fn report_check(json: bool, hr: Option<i32>) {
    match (json, hr) {
        (true, Some(hr)) => println!(
            "{{\"kind\":\"check\",\"name\":\"XamlCheckProcessRequirements\",\"hr\":\"0x{:08x}\"}}",
            hr as u32
        ),
        (true, None) => println!(
            "{{\"kind\":\"check\",\"name\":\"XamlCheckProcessRequirements\",\"present\":false}}"
        ),
        (false, Some(hr)) => println!("XamlCheckProcessRequirements  hr=0x{:08x}", hr as u32),
        (false, None) => println!("XamlCheckProcessRequirements  missing"),
    }
}

fn report_export(json: bool, name: &str, present: bool) {
    if json {
        println!("{{\"kind\":\"export\",\"name\":\"{name}\",\"present\":{present}}}");
    } else if present {
        println!("export {name}  present");
    } else {
        println!("export {name}  missing");
    }
}

fn report_class(json: bool, name: &str, outcome: &Outcome) {
    let factory = outcome.factory;
    let instance = outcome.instance;
    let hr = outcome.hr as u32;
    if json {
        println!(
            "{{\"kind\":\"class\",\"name\":\"{name}\",\"factory\":\"0x{factory:x}\",\"instance\":\"0x{instance:x}\",\"hr\":\"0x{hr:08x}\"}}"
        );
    } else {
        println!("{name}  factory=0x{factory:x}  instance=0x{instance:x}  hr=0x{hr:08x}");
    }
}

/// JSON strings need their backslashes and quotes escaped; the DLL path is user supplied.
fn escaped(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
