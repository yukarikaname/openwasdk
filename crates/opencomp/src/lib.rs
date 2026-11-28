//! opencomp takes the place of `dcompi.dll`, the `Microsoft.UI.Composition` server.
//!
//! The goal is to run the official `Microsoft.UI.Xaml` binaries without DWM or DirectComposition:
//! the XAML layer asks this DLL for visuals, surfaces and animations, and this DLL draws them.
//!
//! The generated metadata lives in `bindings`: interface definitions with their IIDs and the
//! `*_Impl` traits a class implements. It is produced by `comptrace generate` and only compiles
//! for a Windows target (the class helpers call into `windows-core`'s Windows-only factory cache).
//!
//! The crate-level allows are for the ABI: method names like `CreateContainerVisual` are fixed by
//! the metadata, and the generated module has its own spelling conventions.

// The crate is the ABI in front of a Windows engine: on any other host it is empty, so
// `cargo check --workspace` and `cargo test --workspace` still cover everything around it.
#![cfg(windows)]
#![allow(
    non_snake_case,
    non_camel_case_types,
    non_upper_case_globals,
    dead_code
)]

#[cfg(windows)]
mod activation;
#[cfg(windows)]
mod animation;
#[cfg(windows)]
mod animation_controller;
#[cfg(windows)]
mod animation_group;
#[cfg(windows)]
mod batch;
#[cfg(windows)]
// Generated from metadata by `comptrace generate`; its lints are the generator's to fix. It is
// public so a host can name the interfaces it activates by name and build a tree through them.
#[allow(clippy::all)]
pub mod bindings;
#[cfg(windows)]
mod brush;
mod classes;
#[cfg(windows)]
mod clip;
mod clock;
#[cfg(windows)]
mod collection;
#[cfg(windows)]
mod color_animation;
#[cfg(windows)]
mod composite;
mod compositor;
#[cfg(windows)]
mod compositor_ext;
mod compositor_private;
#[cfg(windows)]
pub mod content;
#[cfg(windows)]
mod d2d;
#[cfg(windows)]
mod d2d_gradient;
#[cfg(windows)]
mod d2d_shape;
#[cfg(windows)]
mod easing;
#[cfg(windows)]
mod geometry;
#[cfg(windows)]
mod gradient;
#[cfg(windows)]
mod gradient_collection;
mod identity;
#[cfg(windows)]
mod interop;
pub mod node;
#[cfg(windows)]
mod object;
#[cfg(windows)]
mod present;
#[cfg(windows)]
mod propertyset;
#[cfg(windows)]
mod shape;
#[cfg(windows)]
mod shape_collection;
#[cfg(windows)]
mod step_easing;

#[cfg(windows)]
pub use d2d::frames_presented;
#[cfg(windows)]
pub use d2d::opencomp_frames_presented;
#[cfg(windows)]
pub mod surface;
#[cfg(windows)]
mod visual;

mod factory;
#[cfg(windows)]
mod private_statics;
#[cfg(windows)]
mod qi_trace;

use std::ffi::c_void;

use classes::MANIFEST_CLASSES;

use windows_core::HSTRING;

const S_OK: i32 = 0;
const S_FALSE: i32 = 1;
const E_NOTIMPL: i32 = 0x8000_4001u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const CLASS_E_CLASSNOTAVAILABLE: i32 = 0x8004_0111u32 as i32;

/// The Windows runtime entry points, for a host that reaches the DLL by name.
#[cfg(windows)]
pub use activation::{activate, factory_of, register, unregistered};

/// There is nothing to register with off Windows; the runtime this crate plugs
/// into is combase. The no-op keeps a client's startup path one shape.
#[cfg(not(windows))]
pub fn register() -> windows_core::Result<()> {
    Ok(())
}

/// The runtime classes this DLL answers for.
///
/// In the shipped package the Content classes are served by `Microsoft.ui.xaml.dll`, which
/// carries their activation strings. We serve them here for the composition-only host; this
/// array is the only place that says which DLL owns them.
const CLASSES: &[&str] = &[
    "Microsoft.UI.Composition.Compositor",
    "Microsoft.UI.Content.ContentIsland",
    "Microsoft.UI.Content.DesktopChildSiteBridge",
];

/// What the factory should create for a class name.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Class {
    Compositor,
    /// `CompositionEasingFunction`: a static class, reached through its own statics.
    EasingFunction,
    ContentIsland,
    DesktopChildSiteBridge,
    /// A class this DLL answers for but does not implement: it refuses activation with
    /// `E_NOTIMPL` instead of leaving combase to fail the whole process.
    Unmodelled,
}

impl Class {
    /// The name combase, a manifest or a host uses for this class.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Class::Compositor => "Microsoft.UI.Composition.Compositor",
            Class::EasingFunction => "Microsoft.UI.Composition.CompositionEasingFunction",
            Class::ContentIsland => "Microsoft.UI.Content.ContentIsland",
            Class::DesktopChildSiteBridge => "Microsoft.UI.Content.DesktopChildSiteBridge",
            // A stub factory reports no runtime class name; it makes nothing.
            Class::Unmodelled => "",
        }
    }
}

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
    let name = name.to_string_lossy();
    let Some(class) = class_of(&name) else {
        unsafe { *factory = std::ptr::null_mut() };
        return CLASS_E_CLASSNOTAVAILABLE;
    };
    unsafe { *factory = factory::Factory::create(class) };
    S_OK
}

/// The lifted compositor device entry point, forwarded by `dcompi`'s
/// `WinUIDCompositionCreateDevice3`.
#[unsafe(no_mangle)]
pub extern "system" fn OpenCompRecordingDevice() -> *mut c_void {
    crate::private_statics::recording_device()
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
    class_of(class).is_some()
}

/// The class a runtime class name names, if this DLL serves it.
///
/// This is the one place that maps a name to a class, so the export and the
/// in-process registration cannot disagree about which classes exist.
fn class_of(name: &str) -> Option<Class> {
    if !MANIFEST_CLASSES.contains(&name) && !name.starts_with("Microsoft.UI.Content.") {
        return None;
    }
    match name {
        "Microsoft.UI.Composition.Compositor" => Some(Class::Compositor),
        "Microsoft.UI.Composition.CompositionEasingFunction" => Some(Class::EasingFunction),
        "Microsoft.UI.Content.ContentIsland" => Some(Class::ContentIsland),
        "Microsoft.UI.Content.DesktopChildSiteBridge" => Some(Class::DesktopChildSiteBridge),
        _ => Some(Class::Unmodelled),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A name in the manifest that this DLL does not recognise is a process failure under
    /// Wine, not a return code, so the table and the router must agree.
    #[test]
    fn every_manifest_class_is_served() {
        for name in MANIFEST_CLASSES {
            assert!(class_of(name).is_some(), "{name} is not served");
        }
    }

    #[test]
    fn the_router_knows_compositor_and_rejects_the_rest() {
        assert!(known("Microsoft.UI.Composition.Compositor"));
        assert!(known("Microsoft.UI.Content.ContentIsland"));
        assert!(known("Microsoft.UI.Content.DesktopChildSiteBridge"));
        // A visual is a class the manifest names, so it is served -- as an unmodelled
        // factory that refuses activation, which is a return code the engine survives.
        assert_eq!(
            class_of("Microsoft.UI.Composition.Visual"),
            Some(Class::Unmodelled)
        );
        assert!(!known("Microsoft.UI.Composition.NotAClass"));
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
    fn a_class_the_manifest_does_not_name_is_reported_as_missing() {
        let name = HSTRING::from("Microsoft.UI.Composition.NotAClass");
        let mut factory: *mut c_void = std::ptr::null_mut();
        let result = unsafe { DllGetActivationFactory(handle(&name), &mut factory) };
        assert_eq!(result, CLASS_E_CLASSNOTAVAILABLE);
        assert!(factory.is_null());
    }

    /// A class the manifest names gets a factory even when nothing is implemented: Wine's
    /// combase ends the process on a missing one, so the refusal has to come from
    /// `ActivateInstance`.
    #[test]
    fn a_manifest_class_without_an_implementation_still_gets_a_factory() {
        let name = HSTRING::from("Microsoft.UI.Composition.Visual");
        let mut factory: *mut c_void = std::ptr::null_mut();
        let result = unsafe { DllGetActivationFactory(handle(&name), &mut factory) };
        assert_eq!(result, S_OK);
        assert!(!factory.is_null());
        unsafe { crate::factory::release_for_test(factory) };
    }
}

/// The ABI as a client sees it: activate the class, call the vtable, look at the tree.
#[cfg(all(test, windows))]
mod abi {
    use crate::bindings::Microsoft::UI::Composition::{
        CompositionBrush, ICompositor, IContainerVisual, ISpriteVisual, IVisual, IVisual2,
        InsetClip, Visual,
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
    fn a_relative_root_fills_the_host() {
        let compositor = compositor();
        let root = compositor.CreateContainerVisual().unwrap();
        let visual: IVisual2 = root.cast().unwrap();
        visual
            .SetRelativeSizeAdjustment(Vector2 { x: 1.0, y: 1.0 })
            .unwrap();

        let node = crate::node::resolve(&root).unwrap();
        let tree = crate::node::snapshot_in(&node, Vector2 { x: 800.0, y: 600.0 }).unwrap();
        assert_eq!(tree.size, [800.0, 600.0]);
    }

    #[test]
    fn an_unset_brush_and_clip_still_answer() {
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        let visual: IVisual = sprite.cast().unwrap();
        // Both properties are nullable in the ABI; the trait cannot say so, so a client that
        // reads one back must still get an object rather than an exception.
        assert!(visual.Clip().is_ok());
        let sprite: ISpriteVisual = sprite.cast().unwrap();
        assert!(sprite.Brush().is_ok());
    }

    #[test]
    fn children_keep_the_order_they_were_inserted_in() {
        let compositor = compositor();
        let parent = compositor.CreateContainerVisual().unwrap();
        let container: IContainerVisual = parent.cast().unwrap();
        let collection = container.Children().unwrap();
        let collection: crate::bindings::Microsoft::UI::Composition::IVisualCollection =
            collection.cast().unwrap();
        let first = compositor.CreateSpriteVisual().unwrap();
        let top = compositor.CreateSpriteVisual().unwrap();
        let between = compositor.CreateSpriteVisual().unwrap();

        collection.InsertAtTop(&first).unwrap();
        collection.InsertAtTop(&top).unwrap();
        collection.InsertBelow(&between, &top).unwrap();

        let parent_node = crate::node::resolve(&parent).unwrap();
        let nodes = [
            crate::node::resolve(&first).unwrap(),
            crate::node::resolve(&between).unwrap(),
            crate::node::resolve(&top).unwrap(),
        ];
        let children = parent_node.lock().unwrap().children.clone();
        assert_eq!(children.len(), 3);
        for (child, expected) in children.iter().zip(&nodes) {
            assert!(std::sync::Arc::ptr_eq(child, expected));
        }

        collection.Remove(&between).unwrap();
        assert_eq!(collection.Count().unwrap(), 2);
    }

    #[test]
    fn a_parent_for_transform_round_trips() {
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        let parent = compositor.CreateContainerVisual().unwrap();
        let visual: IVisual2 = sprite.cast().unwrap();
        // Nullable in the ABI, so an unset reference still reports `E_NOTIMPL`.
        assert!(visual.ParentForTransform().is_err());

        let parent_visual: Visual = parent.cast().unwrap();
        visual.SetParentForTransform(&parent_visual).unwrap();
        let stored = visual.ParentForTransform().unwrap();
        assert_eq!(
            Interface::as_raw(&stored),
            Interface::as_raw(&parent_visual)
        );
    }

    #[test]
    fn every_object_can_name_its_compositor() {
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        let object: crate::bindings::Microsoft::UI::Composition::ICompositionObject =
            sprite.cast().unwrap();
        let named = object.Compositor().unwrap();
        assert!(named.cast::<ICompositor>().is_ok());
    }

    #[test]
    fn the_tree_becomes_a_draw_list() {
        let compositor = compositor();
        let root = compositor.CreateContainerVisual().unwrap();
        let sprite = compositor.CreateSpriteVisual().unwrap();

        let visual: IVisual = sprite.cast().unwrap();
        visual.SetSize(Vector2 { x: 8.0, y: 8.0 }).unwrap();
        let brush: CompositionBrush = compositor
            .CreateColorBrushWithColor(Color {
                a: 255,
                r: 255,
                g: 0,
                b: 0,
            })
            .unwrap()
            .cast()
            .unwrap();
        let sprite_visual: ISpriteVisual = sprite.cast().unwrap();
        sprite_visual.SetBrush(&brush).unwrap();
        let children: IContainerVisual = root.cast().unwrap();
        children.Children().unwrap().InsertAtTop(&sprite).unwrap();

        let node = crate::node::resolve(&root).unwrap();
        let tree = crate::node::snapshot_in(&node, Vector2 { x: 16.0, y: 16.0 }).unwrap();
        let draws = tree.draws();
        assert_eq!(draws.len(), 1);
        // Linear light: the brush's 255 is 1.0 after the sRGB decode.
        assert_eq!(draws[0].color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(draws[0].corners[0], [0.0, 0.0]);
        assert_eq!(draws[0].corners[2], [8.0, 8.0]);
    }

    #[test]
    fn an_object_only_answers_for_what_it_is() {
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        assert!(sprite.cast::<ISpriteVisual>().is_ok());
        assert!(sprite.cast::<IContainerVisual>().is_err());
    }
}
