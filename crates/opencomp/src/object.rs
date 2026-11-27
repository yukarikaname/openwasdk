//! The trait bodies every composition object shares.
//!
//! `#[windows_core::implement]` generates one `*_Impl` type per struct, so the same ABI trait has
//! to be written once per object. These macros keep that from becoming the bulk of the crate.
//!
//! A macro body resolves names where it is expanded, so every caller imports [`prelude`]. The
//! follow-on interfaces (`ICompositionObject2`, `IVisual2` and friends) are implemented because a
//! client asks for them by `QueryInterface` before it calls anything; answering `E_NOTIMPL` from
//! a method is recoverable, a failed QI is not.

pub(crate) mod prelude {
    // The names the `#[implement]` lists and `visual_trait!` refer to. The trait bodies
    // themselves write their paths out in full, because they expand in several modules.
    pub(crate) use crate::bindings::Microsoft::UI::Composition::{
        CompositionBackfaceVisibility, CompositionBorderMode, CompositionClip,
        CompositionCompositeMode, CompositionPropertySet, ICompositionObject2, ICompositionObject4,
        ICompositionObject5, ISpriteVisual2, ISpriteVisual2_Impl, IVisual_Impl, IVisual2,
        IVisual2_Impl, IVisual3, IVisual3_Impl, IVisual4, IVisual4_Impl, Visual,
    };
    pub(crate) use crate::bindings::Windows::Foundation::Numerics::Quaternion;
    pub(crate) use windows_core::{HSTRING, Ref};
    pub(crate) use windows_numerics::{Matrix4x4, Vector2, Vector3};
}

use windows_core::{Error, HRESULT};

pub(crate) const E_NOTIMPL: HRESULT = HRESULT(0x8000_4001u32 as i32);
pub(crate) const E_FAIL: HRESULT = HRESULT(0x8000_4005u32 as i32);

pub(crate) fn not_implemented<T>() -> windows_core::Result<T> {
    Err(Error::from_hresult(E_NOTIMPL))
}

macro_rules! composition_followups {
    ($impl_type:ident) => {
        impl $crate::bindings::Microsoft::UI::Composition::ICompositionObject2_Impl for $impl_type {
            fn Comment(&self) -> windows_core::Result<::windows_core::HSTRING> {
                $crate::object::not_implemented()
            }
            fn SetComment(&self, _value: &::windows_core::HSTRING) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn ImplicitAnimations(
                &self,
            ) -> windows_core::Result<
                $crate::bindings::Microsoft::UI::Composition::ImplicitAnimationCollection,
            > {
                $crate::object::not_implemented()
            }
            fn SetImplicitAnimations(
                &self,
                _value: ::windows_core::Ref<
                    $crate::bindings::Microsoft::UI::Composition::ImplicitAnimationCollection,
                >,
            ) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn StartAnimationGroup(
                &self,
                _value: ::windows_core::Ref<
                    $crate::bindings::Microsoft::UI::Composition::ICompositionAnimationBase,
                >,
            ) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn StopAnimationGroup(
                &self,
                _value: ::windows_core::Ref<
                    $crate::bindings::Microsoft::UI::Composition::ICompositionAnimationBase,
                >,
            ) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
        }

        impl $crate::bindings::Microsoft::UI::Composition::ICompositionObject4_Impl for $impl_type {
            fn TryGetAnimationController(
                &self,
                _property: &::windows_core::HSTRING,
            ) -> windows_core::Result<
                $crate::bindings::Microsoft::UI::Composition::AnimationController,
            > {
                $crate::object::not_implemented()
            }
        }

        impl $crate::bindings::Microsoft::UI::Composition::ICompositionObject5_Impl for $impl_type {
            fn StartAnimationWithController(
                &self,
                _property: &::windows_core::HSTRING,
                _animation: ::windows_core::Ref<
                    $crate::bindings::Microsoft::UI::Composition::CompositionAnimation,
                >,
                _controller: ::windows_core::Ref<
                    $crate::bindings::Microsoft::UI::Composition::AnimationController,
                >,
            ) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
        }
    };
}

/// `ICompositionObject` and its follow-ons.
///
/// The `node` argument is for objects that own a visual: their `StartAnimation` and
/// `StopAnimation` reach the tree. Everything else accepts an animation and ignores
/// it, which is what the ABI allows.
macro_rules! composition_object {
    ($impl_type:ident, node) => {
        impl $crate::bindings::Microsoft::UI::Composition::ICompositionObject_Impl for $impl_type {
            fn Compositor(
                &self,
            ) -> windows_core::Result<$crate::bindings::Microsoft::UI::Composition::Compositor>
            {
                windows_core::Interface::cast(&$crate::compositor::instance()?)
            }
            fn Properties(
                &self,
            ) -> windows_core::Result<
                $crate::bindings::Microsoft::UI::Composition::CompositionPropertySet,
            > {
                $crate::propertyset::create()
            }
            fn StartAnimation(
                &self,
                property: &::windows_core::HSTRING,
                animation: ::windows_core::Ref<
                    $crate::bindings::Microsoft::UI::Composition::CompositionAnimation,
                >,
            ) -> windows_core::Result<()> {
                let name = property.to_string_lossy();
                if let (Some(property), Some(keyframes)) = (
                    $crate::node::Property::parse(&name),
                    animation.as_ref().and_then($crate::animation::keyframes),
                ) {
                    $crate::node::start_animation(&self.node, property, keyframes);
                }
                Ok(())
            }
            fn StopAnimation(
                &self,
                property: &::windows_core::HSTRING,
            ) -> windows_core::Result<()> {
                if let Some(property) = $crate::node::Property::parse(&property.to_string_lossy()) {
                    $crate::node::stop_animation(&self.node, property);
                }
                Ok(())
            }
        }
        $crate::object::composition_followups!($impl_type);
    };
    ($impl_type:ident) => {
        impl $crate::bindings::Microsoft::UI::Composition::ICompositionObject_Impl for $impl_type {
            fn Compositor(
                &self,
            ) -> windows_core::Result<$crate::bindings::Microsoft::UI::Composition::Compositor>
            {
                windows_core::Interface::cast(&$crate::compositor::instance()?)
            }
            fn Properties(
                &self,
            ) -> windows_core::Result<
                $crate::bindings::Microsoft::UI::Composition::CompositionPropertySet,
            > {
                $crate::propertyset::create()
            }
            fn StartAnimation(
                &self,
                _property: &::windows_core::HSTRING,
                _animation: ::windows_core::Ref<
                    $crate::bindings::Microsoft::UI::Composition::CompositionAnimation,
                >,
            ) -> windows_core::Result<()> {
                Ok(())
            }
            fn StopAnimation(
                &self,
                _property: &::windows_core::HSTRING,
            ) -> windows_core::Result<()> {
                Ok(())
            }
        }
        $crate::object::composition_followups!($impl_type);
    };
}

macro_rules! visual_trait {
    ($impl_type:ident) => {
        impl IVisual_Impl for $impl_type {
            fn IsVisible(&self) -> windows_core::Result<bool> {
                Ok(self.node.lock().unwrap().visible)
            }
            fn SetIsVisible(&self, value: bool) -> windows_core::Result<()> {
                self.node.lock().unwrap().visible = value;
                Ok(())
            }
            fn Offset(&self) -> windows_core::Result<Vector3> {
                Ok(self.node.lock().unwrap().offset)
            }
            fn SetOffset(&self, value: &Vector3) -> windows_core::Result<()> {
                self.node.lock().unwrap().offset = *value;
                Ok(())
            }
            fn Size(&self) -> windows_core::Result<Vector2> {
                Ok(self.node.lock().unwrap().size)
            }
            fn SetSize(&self, value: &Vector2) -> windows_core::Result<()> {
                self.node.lock().unwrap().size = *value;
                Ok(())
            }
            fn Opacity(&self) -> windows_core::Result<f32> {
                Ok(self.node.lock().unwrap().opacity)
            }
            fn SetOpacity(&self, value: f32) -> windows_core::Result<()> {
                self.node.lock().unwrap().opacity = value.clamp(0.0, 1.0);
                Ok(())
            }
            fn Clip(&self) -> windows_core::Result<CompositionClip> {
                match self.clip.lock().unwrap().clone() {
                    Some(clip) => Ok(clip),
                    None => $crate::clip::empty(),
                }
            }
            fn SetClip(&self, value: Ref<CompositionClip>) -> windows_core::Result<()> {
                self.node.lock().unwrap().insets = value.as_ref().and_then($crate::clip::insets);
                *self.clip.lock().unwrap() = value.cloned();
                Ok(())
            }

            // Properties the renderer does not use yet. A setter that throws would abort a
            // client that only wanted to configure the tree, so these are accepted and dropped;
            // the getters answer the documented default rather than failing.
            fn AnchorPoint(&self) -> windows_core::Result<Vector2> {
                Ok(self.node.lock().unwrap().anchor)
            }
            fn SetAnchorPoint(&self, value: &Vector2) -> windows_core::Result<()> {
                self.node.lock().unwrap().anchor = *value;
                Ok(())
            }
            fn BackfaceVisibility(&self) -> windows_core::Result<CompositionBackfaceVisibility> {
                Ok(CompositionBackfaceVisibility::Inherit)
            }
            fn SetBackfaceVisibility(
                &self,
                _value: CompositionBackfaceVisibility,
            ) -> windows_core::Result<()> {
                Ok(())
            }
            fn BorderMode(&self) -> windows_core::Result<CompositionBorderMode> {
                Ok(CompositionBorderMode::Inherit)
            }
            fn SetBorderMode(&self, _value: CompositionBorderMode) -> windows_core::Result<()> {
                Ok(())
            }
            fn CenterPoint(&self) -> windows_core::Result<Vector3> {
                Ok(self.node.lock().unwrap().center)
            }
            fn SetCenterPoint(&self, value: &Vector3) -> windows_core::Result<()> {
                self.node.lock().unwrap().center = *value;
                Ok(())
            }
            fn CompositeMode(&self) -> windows_core::Result<CompositionCompositeMode> {
                Ok(CompositionCompositeMode::Inherit)
            }
            fn SetCompositeMode(
                &self,
                _value: CompositionCompositeMode,
            ) -> windows_core::Result<()> {
                Ok(())
            }
            fn Orientation(&self) -> windows_core::Result<Quaternion> {
                Ok(Quaternion {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 1.0,
                })
            }
            fn SetOrientation(&self, _value: &Quaternion) -> windows_core::Result<()> {
                Ok(())
            }
            fn RotationAngle(&self) -> windows_core::Result<f32> {
                Ok(self.node.lock().unwrap().rotation)
            }
            fn SetRotationAngle(&self, value: f32) -> windows_core::Result<()> {
                self.node.lock().unwrap().rotation = value;
                Ok(())
            }
            fn RotationAngleInDegrees(&self) -> windows_core::Result<f32> {
                Ok(self.node.lock().unwrap().rotation.to_degrees())
            }
            fn SetRotationAngleInDegrees(&self, value: f32) -> windows_core::Result<()> {
                self.node.lock().unwrap().rotation = value.to_radians();
                Ok(())
            }
            fn RotationAxis(&self) -> windows_core::Result<Vector3> {
                Ok(Vector3 {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                })
            }
            fn SetRotationAxis(&self, _value: &Vector3) -> windows_core::Result<()> {
                Ok(())
            }
            fn Scale(&self) -> windows_core::Result<Vector3> {
                Ok(self.node.lock().unwrap().scale)
            }
            fn SetScale(&self, value: &Vector3) -> windows_core::Result<()> {
                self.node.lock().unwrap().scale = *value;
                Ok(())
            }
            fn TransformMatrix(&self) -> windows_core::Result<Matrix4x4> {
                Ok(Matrix4x4 {
                    m11: 1.0,
                    m12: 0.0,
                    m13: 0.0,
                    m14: 0.0,
                    m21: 0.0,
                    m22: 1.0,
                    m23: 0.0,
                    m24: 0.0,
                    m31: 0.0,
                    m32: 0.0,
                    m33: 1.0,
                    m34: 0.0,
                    m41: 0.0,
                    m42: 0.0,
                    m43: 0.0,
                    m44: 1.0,
                })
            }
            fn SetTransformMatrix(&self, _value: &Matrix4x4) -> windows_core::Result<()> {
                Ok(())
            }
            // The parent link is real state, not a property, and nothing builds it yet.
            fn Parent(
                &self,
            ) -> windows_core::Result<$crate::bindings::Microsoft::UI::Composition::ContainerVisual>
            {
                $crate::object::not_implemented()
            }
        }

        impl IVisual2_Impl for $impl_type {
            fn ParentForTransform(&self) -> windows_core::Result<Visual> {
                $crate::object::not_implemented()
            }
            fn SetParentForTransform(&self, _value: Ref<Visual>) -> windows_core::Result<()> {
                Ok(())
            }
            fn RelativeOffsetAdjustment(&self) -> windows_core::Result<Vector3> {
                Ok(self
                    .node
                    .lock()
                    .unwrap()
                    .relative_offset
                    .unwrap_or_default())
            }
            fn SetRelativeOffsetAdjustment(&self, value: &Vector3) -> windows_core::Result<()> {
                self.node.lock().unwrap().relative_offset = Some(*value);
                Ok(())
            }
            fn RelativeSizeAdjustment(&self) -> windows_core::Result<Vector2> {
                Ok(self
                    .node
                    .lock()
                    .unwrap()
                    .relative_size
                    .unwrap_or(Vector2 { x: 1.0, y: 1.0 }))
            }
            fn SetRelativeSizeAdjustment(&self, value: &Vector2) -> windows_core::Result<()> {
                self.node.lock().unwrap().relative_size = Some(*value);
                Ok(())
            }
        }

        impl IVisual3_Impl for $impl_type {
            fn IsHitTestVisible(&self) -> windows_core::Result<bool> {
                Ok(self.node.lock().unwrap().hit_test_visible)
            }
            fn SetIsHitTestVisible(&self, value: bool) -> windows_core::Result<()> {
                self.node.lock().unwrap().hit_test_visible = value;
                Ok(())
            }
        }

        impl IVisual4_Impl for $impl_type {
            // A hint the renderer does not act on yet; keeping the value round-trips.
            fn IsPixelSnappingEnabled(&self) -> windows_core::Result<bool> {
                Ok(false)
            }
            fn SetIsPixelSnappingEnabled(&self, _value: bool) -> windows_core::Result<()> {
                Ok(())
            }
        }
    };
}

pub(crate) use composition_followups;
pub(crate) use composition_object;
pub(crate) use visual_trait;
