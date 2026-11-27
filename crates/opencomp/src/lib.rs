//! opencomp replaces `Microsoft.UI.Composition.dll` with an ABI-compatible WinRT server.
//!
//! The goal is to run the official `Microsoft.UI.Xaml` binaries without DWM or DirectComposition:
//! the XAML layer asks this DLL for visuals, surfaces and animations, and this DLL draws them.
//!
//! The generated metadata lives in `bindings`: interface definitions with their IIDs and the
//! `*_Impl` traits a class implements. It is produced by `comptrace generate` and only compiles
//! for a Windows target (the class helpers call into `windows-core`'s Windows-only factory cache).

#![allow(
    non_snake_case,
    non_camel_case_types,
    non_upper_case_globals,
    dead_code
)]

#[cfg(windows)]
mod bindings;
#[cfg(windows)]
mod brush;
#[cfg(windows)]
mod clip;
#[cfg(windows)]
mod collection;
#[cfg(windows)]
mod compositor;
mod node;
#[cfg(windows)]
mod object;
#[cfg(windows)]
mod visual;

mod factory;

use std::ffi::c_void;

use windows_core::HSTRING;

const S_OK: i32 = 0;
const S_FALSE: i32 = 1;
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const CLASS_E_CLASSNOTAVAILABLE: i32 = 0x8004_0111u32 as i32;

/// The runtime classes this DLL answers for.
const CLASSES: &[&str] = &["Microsoft.UI.Composition.Compositor"];

/// WinRT entry point: return the activation factory for `class`.
///
/// # Safety
/// `class` is an `HSTRING` owned by the caller and `factory` is writable for one pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetActivationFactory(
    class: *mut c_void,
    factory: *mut *mut c_void,
) -> i32 {
    if class.is_null() || factory.is_null() {
        return E_POINTER;
    }
    // The parameter is the HSTRING handle itself, which is a pointer to the string header. Bit
    // copy it into an HSTRING and keep it from dropping: the caller owns the reference.
    let name = std::mem::ManuallyDrop::new(unsafe {
        std::mem::transmute_copy::<*mut c_void, HSTRING>(&class)
    });
    if !known(&name.to_string_lossy()) {
        unsafe { *factory = std::ptr::null_mut() };
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    unsafe { *factory = factory::Factory::create() };
    S_OK
}

/// A DLL with no global objects can always be unloaded.
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> i32 {
    S_FALSE
}

/// Classic COM activation is not part of WinRT; only the factory entry point is.
#[unsafe(no_mangle)]
pub extern "system" fn DllGetClassObject(
    _class: *const c_void,
    _iid: *const c_void,
    _out: *mut *mut c_void,
) -> i32 {
    E_NOTIMPL
}

fn known(class: &str) -> bool {
    CLASSES.contains(&class)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_router_knows_compositor_and_rejects_the_rest() {
        assert!(known("Microsoft.UI.Composition.Compositor"));
        assert!(!known("Microsoft.UI.Composition.Visual"));
        assert!(!known(""));
    }
}

#[cfg(test)]
mod export_tests {
    use super::*;

    /// WinRT passes the HSTRING handle by value, which is the pointer inside the HSTRING.
    fn handle(name: &HSTRING) -> *mut c_void {
        unsafe { std::mem::transmute_copy(name) }
    }

    #[test]
    fn a_known_class_gets_a_factory() {
        let name = HSTRING::from("Microsoft.UI.Composition.Compositor");
        let mut factory: *mut c_void = std::ptr::null_mut();
        let result = unsafe { DllGetActivationFactory(handle(&name), &mut factory) };
        assert_eq!(result, S_OK);
        assert!(!factory.is_null());
        unsafe { crate::factory::release_for_test(factory) };
    }

    #[test]
    fn a_class_this_dll_does_not_serve_is_reported_as_missing() {
        let name = HSTRING::from("Microsoft.UI.Composition.Visual");
        let mut factory: *mut c_void = std::ptr::null_mut();
        let result = unsafe { DllGetActivationFactory(handle(&name), &mut factory) };
        assert_eq!(result, CLASS_E_CLASSNOTAVAILABLE);
        assert!(factory.is_null());
    }
}

/// The ABI as a client sees it: activate the class, call the vtable, look at the tree.
#[cfg(all(test, windows))]
mod abi {
    use crate::bindings::Microsoft::UI::Composition::{
        CompositionBrush, ICompositor, IContainerVisual, ISpriteVisual, IVisual, InsetClip,
    };
    use crate::bindings::Windows::UI::Color;
    use windows_core::Interface;
    use windows_numerics::{Vector2, Vector3};

    fn compositor() -> ICompositor {
        crate::compositor::instance().unwrap().cast().unwrap()
    }

    #[test]
    fn a_sprite_becomes_a_child_of_the_container() {
        let compositor = compositor();
        let container = compositor.CreateContainerVisual().unwrap();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        let visual = sprite.cast::<IVisual>().unwrap();
        visual
            .SetOffset(Vector3 {
                x: 4.0,
                y: 8.0,
                z: 0.0,
            })
            .unwrap();
        visual.SetSize(Vector2 { x: 10.0, y: 20.0 }).unwrap();
        let children: IContainerVisual = container.cast().unwrap();
        children.Children().unwrap().InsertAtTop(&sprite).unwrap();

        let node = crate::node::resolve(&container).unwrap();
        let tree = crate::node::snapshot(&node).unwrap();
        assert_eq!(tree.children.len(), 1);
        assert_eq!(tree.children[0].offset, [4.0, 8.0]);
        assert_eq!(tree.children[0].size, [10.0, 20.0]);
    }

    #[test]
    fn a_colour_brush_paints_the_sprite() {
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        let brush = compositor
            .CreateColorBrushWithColor(Color {
                a: 255,
                r: 255,
                g: 0,
                b: 0,
            })
            .unwrap();
        let brush: CompositionBrush = brush.cast().unwrap();
        let sprite_visual: ISpriteVisual = sprite.cast().unwrap();
        sprite_visual.SetBrush(&brush).unwrap();

        let node = crate::node::resolve(&sprite).unwrap();
        let tree = crate::node::snapshot(&node).unwrap();
        assert_eq!(tree.color, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn an_inset_clip_reaches_the_visual() {
        let compositor = compositor();
        let container = compositor.CreateContainerVisual().unwrap();
        let visual = container.cast::<IVisual>().unwrap();
        visual.SetSize(Vector2 { x: 100.0, y: 50.0 }).unwrap();
        let clip: InsetClip = compositor
            .CreateInsetClipWithInsets(10.0, 5.0, 20.0, 5.0)
            .unwrap();
        let clip: crate::bindings::Microsoft::UI::Composition::CompositionClip =
            clip.cast().unwrap();
        visual.SetClip(&clip).unwrap();

        let node = crate::node::resolve(&container).unwrap();
        let tree = crate::node::snapshot(&node).unwrap();
        assert_eq!(tree.clip, Some((10.0, 5.0, 70.0, 40.0)));
    }

    #[test]
    fn an_object_only_answers_for_what_it_is() {
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        assert!(sprite.cast::<ISpriteVisual>().is_ok());
        assert!(sprite.cast::<IContainerVisual>().is_err());
    }
}
