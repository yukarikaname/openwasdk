//! Input listener bookkeeping and the input settings stores.
//!
//! The App SDK's flat C layer carries opaque listener objects, not callbacks.
//! The register/unregister pair keeps the pointer the caller handed over, and
//! the same pointer is the token that unregisters it. `WindowPositionChanged_*`
//! carries one object that identifies the window; `InputPaneFramework_Advise`
//! is the one export here with a separate out cookie.
//!
//! Injecting a back-button press needs the private handler interface the
//! listener implements, which this flat layer does not define. With no
//! listener the answer is `S_FALSE`; with one it is `E_NOTIMPL` rather than an
//! invented vtable call.

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{E_INVALIDARG, E_NOTIMPL, E_POINTER, S_OK};

/// A successful `HRESULT` that also says "no listener took the event".
const S_FALSE: i32 = 1;

/// A pointer kept as an identity, never dereferenced.
type Opaque = usize;

/// The settings bits the export rejects when set; taken from the DLL's own
/// fallback for `TextInputProxy_putInputSettings`.
const INPUT_SETTINGS_MASK: i32 = 0x87f3_f000u32 as i32;

struct Registry {
    /// (key, listener): the key is the owner for the per-owner pairs and the
    /// listener itself for the pairs that unregister by listener pointer.
    entries: Vec<(Opaque, Opaque)>,
}

impl Registry {
    const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn insert(&mut self, key: Opaque, listener: Opaque) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.0 == key) {
            entry.1 = listener;
        } else {
            self.entries.push((key, listener));
        }
    }

    fn remove(&mut self, key: Opaque) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.0 != key);
        self.entries.len() != before
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

static BACK_BUTTON: Mutex<Registry> = Mutex::new(Registry::new());
static FRAMEWORK_INPUT_VIEW: Mutex<Registry> = Mutex::new(Registry::new());
static INPUT_PANE: Mutex<Registry> = Mutex::new(Registry::new());
static WINDOW_POSITION: Mutex<Registry> = Mutex::new(Registry::new());

static NEXT_COOKIE: AtomicU64 = AtomicU64::new(1);

static KEYBOARD_SKIN: Mutex<Option<i32>> = Mutex::new(None);
static INPUT_SETTINGS: Mutex<Option<i32>> = Mutex::new(None);

fn insert(registry: &Mutex<Registry>, key: *mut c_void, listener: *mut c_void) -> bool {
    if key.is_null() || listener.is_null() {
        return false;
    }
    registry
        .lock()
        .unwrap()
        .insert(key as usize, listener as usize);
    true
}

fn remove(registry: &Mutex<Registry>, key: *mut c_void) -> i32 {
    if key.is_null() {
        return E_INVALIDARG;
    }
    if registry.lock().unwrap().remove(key as usize) {
        S_OK
    } else {
        E_INVALIDARG
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn BackButtonIntegration_RegisterListener(listener: *mut c_void) -> i32 {
    if insert(&BACK_BUTTON, listener, listener) {
        S_OK
    } else {
        E_INVALIDARG
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn BackButtonIntegration_UnregisterListener(listener: *mut c_void) -> i32 {
    remove(&BACK_BUTTON, listener)
}

/// Deliver a back-button press to the registered listeners.
///
/// The listener is an opaque COM object and its handler interface is not part
/// of this flat layer, so a non-empty list answers `E_NOTIMPL` and leaves the
/// caller's flag alone. An empty list writes false and says `S_FALSE`.
#[unsafe(no_mangle)]
pub extern "system" fn BackButtonIntegration_InjectBackButtonPress(handled: *mut i32) -> i32 {
    if handled.is_null() {
        return E_POINTER;
    }
    if !BACK_BUTTON.lock().unwrap().is_empty() {
        return E_NOTIMPL;
    }
    // Safety: `handled` was checked for null.
    unsafe { *handled = 0 };
    S_FALSE
}

#[unsafe(no_mangle)]
pub extern "system" fn FrameworkInputView_RegisterListener(
    owner: *mut c_void,
    listener: *mut c_void,
) -> i32 {
    if insert(&FRAMEWORK_INPUT_VIEW, owner, listener) {
        S_OK
    } else {
        E_INVALIDARG
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn FrameworkInputView_UnregisterListener(owner: *mut c_void) -> i32 {
    remove(&FRAMEWORK_INPUT_VIEW, owner)
}

#[unsafe(no_mangle)]
pub extern "system" fn InputPaneFramework_RegisterListener(
    owner: *mut c_void,
    listener: *mut c_void,
) -> i32 {
    if insert(&INPUT_PANE, owner, listener) {
        S_OK
    } else {
        E_INVALIDARG
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn InputPaneFramework_UnregisterListener(owner: *mut c_void) -> i32 {
    remove(&INPUT_PANE, owner)
}

/// Advise the input pane for `pane` and hand the caller a cookie.
///
/// `window` is the pane's window context; this layer keys the registration by
/// the pane, which is what `InputPaneFramework_UnregisterListener` carries.
#[unsafe(no_mangle)]
pub extern "system" fn InputPaneFramework_Advise(
    pane: *mut c_void,
    _window: *mut c_void,
    handler: *mut c_void,
    cookie: *mut u64,
) -> i32 {
    if pane.is_null() || handler.is_null() {
        return E_INVALIDARG;
    }
    if cookie.is_null() {
        return E_POINTER;
    }
    INPUT_PANE
        .lock()
        .unwrap()
        .insert(pane as usize, handler as usize);
    let value = NEXT_COOKIE.fetch_add(1, Ordering::Relaxed);
    // Safety: `cookie` was checked for null.
    unsafe { *cookie = value };
    S_OK
}

#[unsafe(no_mangle)]
pub extern "system" fn WindowPositionChanged_RegisterListener(window: *mut c_void) -> i32 {
    if insert(&WINDOW_POSITION, window, window) {
        S_OK
    } else {
        E_INVALIDARG
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn WindowPositionChanged_UnregisterListener(window: *mut c_void) -> i32 {
    remove(&WINDOW_POSITION, window)
}

/// The exported skin value is the pair (0, 1); anything else is rejected by
/// the DLL's own fallback. The target object is not needed by this store.
#[unsafe(no_mangle)]
pub extern "system" fn TextInput_SetKeyboardSkin(skin: i32, _target: *mut c_void) -> i32 {
    if skin != 0 && skin != 1 {
        return E_INVALIDARG;
    }
    *KEYBOARD_SKIN.lock().unwrap() = Some(skin);
    S_OK
}

/// The export carries a target object, not the value to set, and this shim
/// has no text-prediction implementation to change that object with.
#[unsafe(no_mangle)]
pub extern "system" fn TextInput_SetTextPrediction(_target: *mut c_void) -> i32 {
    E_NOTIMPL
}

#[unsafe(no_mangle)]
pub extern "system" fn TextInputProxy_putInputSettings(_target: *mut c_void, settings: i32) -> i32 {
    if settings & INPUT_SETTINGS_MASK != 0 {
        return E_INVALIDARG;
    }
    *INPUT_SETTINGS.lock().unwrap() = Some(settings);
    S_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn count(registry: &Mutex<Registry>) -> usize {
        registry.lock().unwrap().entries.len()
    }

    fn clear() {
        for registry in [
            &BACK_BUTTON,
            &FRAMEWORK_INPUT_VIEW,
            &INPUT_PANE,
            &WINDOW_POSITION,
        ] {
            registry.lock().unwrap().entries.clear();
        }
        NEXT_COOKIE.store(1, Ordering::SeqCst);
        *KEYBOARD_SKIN.lock().unwrap() = None;
        *INPUT_SETTINGS.lock().unwrap() = None;
    }

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_LOCK.lock().unwrap();
        clear();
        guard
    }

    fn pointer(value: usize) -> *mut c_void {
        value as *mut c_void
    }

    #[test]
    fn the_back_button_pair_counts_and_forgets() {
        let _guard = lock();
        let first = pointer(0x1000);
        let second = pointer(0x2000);
        assert_eq!(BackButtonIntegration_RegisterListener(first), S_OK);
        assert_eq!(BackButtonIntegration_RegisterListener(second), S_OK);
        assert_eq!(count(&BACK_BUTTON), 2);

        assert_eq!(BackButtonIntegration_UnregisterListener(first), S_OK);
        assert_eq!(count(&BACK_BUTTON), 1);
        assert_eq!(
            BackButtonIntegration_UnregisterListener(first),
            E_INVALIDARG
        );
        assert_eq!(count(&BACK_BUTTON), 1);

        assert_eq!(BackButtonIntegration_UnregisterListener(second), S_OK);
        assert_eq!(count(&BACK_BUTTON), 0);
        assert_eq!(
            BackButtonIntegration_RegisterListener(std::ptr::null_mut()),
            E_INVALIDARG
        );
    }

    #[test]
    fn injecting_without_a_handler_is_not_handled() {
        let _guard = lock();
        let mut handled = 7;
        assert_eq!(
            BackButtonIntegration_InjectBackButtonPress(&mut handled),
            S_FALSE
        );
        assert_eq!(handled, 0);

        assert_eq!(
            BackButtonIntegration_RegisterListener(pointer(0x1000)),
            S_OK
        );
        handled = 7;
        assert_eq!(
            BackButtonIntegration_InjectBackButtonPress(&mut handled),
            E_NOTIMPL
        );
        assert_eq!(handled, 7);
        assert_eq!(
            BackButtonIntegration_InjectBackButtonPress(std::ptr::null_mut()),
            E_POINTER
        );
    }

    #[test]
    fn the_input_view_and_input_pane_pairs_key_by_owner() {
        let _guard = lock();
        let first_owner = pointer(0x1000);
        let second_owner = pointer(0x2000);
        assert_eq!(
            FrameworkInputView_RegisterListener(first_owner, pointer(0x3000)),
            S_OK
        );
        assert_eq!(
            FrameworkInputView_RegisterListener(first_owner, pointer(0x4000)),
            S_OK
        );
        assert_eq!(
            FrameworkInputView_RegisterListener(second_owner, pointer(0x5000)),
            S_OK
        );
        assert_eq!(count(&FRAMEWORK_INPUT_VIEW), 2);

        assert_eq!(FrameworkInputView_UnregisterListener(first_owner), S_OK);
        assert_eq!(
            FrameworkInputView_UnregisterListener(first_owner),
            E_INVALIDARG
        );
        assert_eq!(count(&FRAMEWORK_INPUT_VIEW), 1);
        assert_eq!(FrameworkInputView_UnregisterListener(second_owner), S_OK);

        assert_eq!(
            InputPaneFramework_RegisterListener(first_owner, pointer(0x3000)),
            S_OK
        );
        assert_eq!(count(&INPUT_PANE), 1);
        assert_eq!(InputPaneFramework_UnregisterListener(first_owner), S_OK);
        assert_eq!(
            InputPaneFramework_UnregisterListener(first_owner),
            E_INVALIDARG
        );
    }

    #[test]
    fn advise_hands_out_a_cookie_and_shares_the_pane_list() {
        let _guard = lock();
        let pane = pointer(0x1000);
        let window = pointer(0x2000);
        let mut first = 0;
        let mut second = 0;
        assert_eq!(
            InputPaneFramework_Advise(pane, window, pointer(0x3000), &mut first),
            S_OK
        );
        assert_ne!(first, 0);
        assert_eq!(count(&INPUT_PANE), 1);
        assert_eq!(
            InputPaneFramework_Advise(pane, window, pointer(0x4000), &mut second),
            S_OK
        );
        assert_ne!(second, first);
        assert_eq!(count(&INPUT_PANE), 1);

        assert_eq!(InputPaneFramework_UnregisterListener(pane), S_OK);
        assert_eq!(InputPaneFramework_UnregisterListener(pane), E_INVALIDARG);

        let mut untouched = 0x99;
        assert_eq!(
            InputPaneFramework_Advise(pane, window, std::ptr::null_mut(), &mut untouched),
            E_INVALIDARG
        );
        assert_eq!(untouched, 0x99);
        assert_eq!(count(&INPUT_PANE), 0);
        assert_eq!(
            InputPaneFramework_Advise(pane, window, pointer(0x3000), std::ptr::null_mut()),
            E_POINTER
        );
        assert_eq!(count(&INPUT_PANE), 0);
    }

    #[test]
    fn the_window_pair_uses_the_object_it_was_given() {
        let _guard = lock();
        let first = pointer(0x1000);
        let second = pointer(0x2000);
        assert_eq!(WindowPositionChanged_RegisterListener(first), S_OK);
        assert_eq!(WindowPositionChanged_RegisterListener(second), S_OK);
        assert_eq!(count(&WINDOW_POSITION), 2);

        assert_eq!(WindowPositionChanged_UnregisterListener(first), S_OK);
        assert_eq!(
            WindowPositionChanged_UnregisterListener(first),
            E_INVALIDARG
        );
        assert_eq!(count(&WINDOW_POSITION), 1);
        assert_eq!(WindowPositionChanged_UnregisterListener(second), S_OK);
        assert_eq!(
            WindowPositionChanged_RegisterListener(std::ptr::null_mut()),
            E_INVALIDARG
        );
    }

    #[test]
    fn the_settings_stores_keep_the_value_they_accepted() {
        let _guard = lock();
        let target = pointer(0x1000);

        assert_eq!(TextInput_SetKeyboardSkin(1, target), S_OK);
        assert_eq!(*KEYBOARD_SKIN.lock().unwrap(), Some(1));
        assert_eq!(TextInput_SetKeyboardSkin(2, target), E_INVALIDARG);
        assert_eq!(*KEYBOARD_SKIN.lock().unwrap(), Some(1));

        assert_eq!(TextInputProxy_putInputSettings(target, 0x100), S_OK);
        assert_eq!(*INPUT_SETTINGS.lock().unwrap(), Some(0x100));
        assert_eq!(
            TextInputProxy_putInputSettings(target, 0x1000),
            E_INVALIDARG
        );
        assert_eq!(*INPUT_SETTINGS.lock().unwrap(), Some(0x100));

        assert_eq!(TextInput_SetTextPrediction(target), E_NOTIMPL);
    }
}
