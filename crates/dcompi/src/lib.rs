//! A `dcompi.dll` that loads.
//!
//! `Microsoft.DirectManipulation.dll` imports exactly one function from the SDK's
//! compositor, `WinUIDCompositionCreateDevice3`, and the shipped file cannot load under
//! Wine: it needs `api-ms-win-appmodel-unlock-l1-1-0.dll!IsDeveloperModeEnabled`, an API
//! set Wine does not carry. The manipulation stack is the input plumbing behind touch and
//! pen, which a probe never feeds, so the export exists and refuses instead of pretending
//! to hand back a device.

const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;

/// `WinUIDCompositionCreateDevice3`: the lifted compositor's device factory.
#[unsafe(no_mangle)]
pub extern "system" fn WinUIDCompositionCreateDevice3() -> i32 {
    if std::env::var_os("OPENCOMP_TRACE").is_some() {
        eprintln!("dcompi: WinUIDCompositionCreateDevice3 refused");
    }
    E_NOTIMPL
}
