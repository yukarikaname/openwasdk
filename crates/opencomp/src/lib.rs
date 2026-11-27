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

#![allow(
    non_snake_case,
    non_camel_case_types,
    non_upper_case_globals,
    dead_code
)]

#[cfg(windows)]
mod animation;
// Generated from metadata by `comptrace generate`; its lints are the generator's to fix.
#[cfg(windows)]
#[allow(clippy::all)]
mod bindings;
#[cfg(windows)]
mod brush;
#[cfg(windows)]
mod clip;
#[cfg(windows)]
mod collection;
#[cfg(windows)]
mod color_animation;
#[cfg(windows)]
mod compositor;
#[cfg(windows)]
mod easing;
mod identity;
mod node;
#[cfg(windows)]
mod object;
#[cfg(windows)]
mod propertyset;
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
        CompositionBrush, ICompositor, IContainerVisual, ISpriteVisual, IVisual, IVisual2,
        InsetClip,
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
    fn every_object_can_name_its_compositor() {
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        let object: crate::bindings::Microsoft::UI::Composition::ICompositionObject =
            sprite.cast().unwrap();
        let named = object.Compositor().unwrap();
        assert!(named.cast::<ICompositor>().is_ok());
    }

    #[test]
    fn the_tree_reaches_the_renderer() {
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
        // A machine without a GPU adapter on the CI runner cannot say anything either way.
        let Some(mut renderer) = opencomp_core::Renderer::headless((16, 16), [0.0, 0.0, 1.0, 1.0])
        else {
            return;
        };
        renderer.set_root(tree);
        let pixels = renderer.render_to_pixels();
        let pixel = |x: usize, y: usize| {
            let at = (y * 16 + x) * 4;
            [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
        };
        assert_eq!(pixel(4, 4), [255, 0, 0, 255]);
        assert_eq!(pixel(12, 12), [0, 0, 255, 255]);
    }

    #[test]
    fn an_easing_function_keeps_its_control_points() {
        let compositor = compositor();
        let easing = compositor
            .CreateCubicBezierEasingFunction(Vector2 { x: 0.1, y: 0.2 }, Vector2 { x: 0.8, y: 0.9 })
            .unwrap();
        let easing: crate::bindings::Microsoft::UI::Composition::ICubicBezierEasingFunction =
            easing.cast().unwrap();
        assert_eq!(easing.ControlPoint1().unwrap(), Vector2 { x: 0.1, y: 0.2 });
        assert_eq!(easing.ControlPoint2().unwrap(), Vector2 { x: 0.8, y: 0.9 });
        assert!(compositor.CreateLinearEasingFunction().is_ok());
    }

    #[test]
    fn an_animation_reaches_the_tree() {
        use crate::bindings::Microsoft::UI::Composition::{
            CompositionAnimation, ICompositionObject, IScalarKeyFrameAnimation,
        };
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        let animation = compositor.CreateScalarKeyFrameAnimation().unwrap();
        let keyframes: IScalarKeyFrameAnimation = animation.cast().unwrap();
        keyframes.InsertKeyFrame(0.0, 0.25).unwrap();
        let animation: CompositionAnimation = animation.cast().unwrap();

        let object: ICompositionObject = sprite.cast().unwrap();
        object.StartAnimation("Opacity", &animation).unwrap();
        let node = crate::node::resolve(&sprite).unwrap();
        assert_eq!(crate::node::snapshot(&node).unwrap().opacity, 0.25);

        object.StopAnimation("Opacity").unwrap();
        assert_eq!(crate::node::snapshot(&node).unwrap().opacity, 1.0);
    }

    #[test]
    fn an_object_only_answers_for_what_it_is() {
        let compositor = compositor();
        let sprite = compositor.CreateSpriteVisual().unwrap();
        assert!(sprite.cast::<ISpriteVisual>().is_ok());
        assert!(sprite.cast::<IContainerVisual>().is_err());
    }
}
