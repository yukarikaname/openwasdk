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
    // Exactly what the macros below name. Callers import this instead of repeating the list,
    // and must not import the same names again: a duplicate import hides the glob and makes the
    // re-export look unused.
    pub(crate) use crate::bindings::Microsoft::UI::Composition::{
        AnimationController, CompositionAnimation, CompositionBackfaceVisibility,
        CompositionBorderMode, CompositionClip, CompositionCompositeMode, CompositionPropertySet,
        Compositor, ICompositionAnimationBase, ICompositionObject_Impl, ICompositionObject2,
        ICompositionObject2_Impl, ICompositionObject4, ICompositionObject4_Impl,
        ICompositionObject5, ICompositionObject5_Impl, ISpriteVisual2, ISpriteVisual2_Impl,
        IVisual_Impl, IVisual2, IVisual2_Impl, IVisual3, IVisual3_Impl, IVisual4, IVisual4_Impl,
        ImplicitAnimationCollection, Visual,
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

macro_rules! composition_object {
    ($impl_type:ident) => {
        impl ICompositionObject_Impl for $impl_type {
            fn Compositor(&self) -> windows_core::Result<Compositor> {
                $crate::object::not_implemented()
            }
            fn Properties(&self) -> windows_core::Result<CompositionPropertySet> {
                $crate::object::not_implemented()
            }
            fn StartAnimation(
                &self,
                _property: &HSTRING,
                _animation: Ref<CompositionAnimation>,
            ) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn StopAnimation(&self, _property: &HSTRING) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
        }

        impl ICompositionObject2_Impl for $impl_type {
            fn Comment(&self) -> windows_core::Result<HSTRING> {
                $crate::object::not_implemented()
            }
            fn SetComment(&self, _value: &HSTRING) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn ImplicitAnimations(&self) -> windows_core::Result<ImplicitAnimationCollection> {
                $crate::object::not_implemented()
            }
            fn SetImplicitAnimations(
                &self,
                _value: Ref<ImplicitAnimationCollection>,
            ) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn StartAnimationGroup(
                &self,
                _value: Ref<ICompositionAnimationBase>,
            ) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn StopAnimationGroup(
                &self,
                _value: Ref<ICompositionAnimationBase>,
            ) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
        }

        impl ICompositionObject4_Impl for $impl_type {
            fn TryGetAnimationController(
                &self,
                _property: &HSTRING,
            ) -> windows_core::Result<AnimationController> {
                $crate::object::not_implemented()
            }
        }

        impl ICompositionObject5_Impl for $impl_type {
            fn StartAnimationWithController(
                &self,
                _property: &HSTRING,
                _animation: Ref<CompositionAnimation>,
                _controller: Ref<AnimationController>,
            ) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
        }
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
                    None => Err(Error::from_hresult($crate::object::E_FAIL)),
                }
            }
            fn SetClip(&self, value: Ref<CompositionClip>) -> windows_core::Result<()> {
                self.node.lock().unwrap().insets = value.as_ref().and_then($crate::clip::insets);
                *self.clip.lock().unwrap() = value.cloned();
                Ok(())
            }

            fn AnchorPoint(&self) -> windows_core::Result<Vector2> {
                $crate::object::not_implemented()
            }
            fn SetAnchorPoint(&self, _value: &Vector2) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn BackfaceVisibility(&self) -> windows_core::Result<CompositionBackfaceVisibility> {
                $crate::object::not_implemented()
            }
            fn SetBackfaceVisibility(
                &self,
                _value: CompositionBackfaceVisibility,
            ) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn BorderMode(&self) -> windows_core::Result<CompositionBorderMode> {
                $crate::object::not_implemented()
            }
            fn SetBorderMode(&self, _value: CompositionBorderMode) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn CenterPoint(&self) -> windows_core::Result<Vector3> {
                $crate::object::not_implemented()
            }
            fn SetCenterPoint(&self, _value: &Vector3) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn CompositeMode(&self) -> windows_core::Result<CompositionCompositeMode> {
                $crate::object::not_implemented()
            }
            fn SetCompositeMode(
                &self,
                _value: CompositionCompositeMode,
            ) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn Orientation(&self) -> windows_core::Result<Quaternion> {
                $crate::object::not_implemented()
            }
            fn SetOrientation(&self, _value: &Quaternion) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn Parent(
                &self,
            ) -> windows_core::Result<$crate::bindings::Microsoft::UI::Composition::ContainerVisual>
            {
                $crate::object::not_implemented()
            }
            fn RotationAngle(&self) -> windows_core::Result<f32> {
                $crate::object::not_implemented()
            }
            fn SetRotationAngle(&self, _value: f32) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn RotationAngleInDegrees(&self) -> windows_core::Result<f32> {
                $crate::object::not_implemented()
            }
            fn SetRotationAngleInDegrees(&self, _value: f32) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn RotationAxis(&self) -> windows_core::Result<Vector3> {
                $crate::object::not_implemented()
            }
            fn SetRotationAxis(&self, _value: &Vector3) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn Scale(&self) -> windows_core::Result<Vector3> {
                $crate::object::not_implemented()
            }
            fn SetScale(&self, _value: &Vector3) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn TransformMatrix(&self) -> windows_core::Result<Matrix4x4> {
                $crate::object::not_implemented()
            }
            fn SetTransformMatrix(&self, _value: &Matrix4x4) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
        }

        impl IVisual2_Impl for $impl_type {
            fn ParentForTransform(&self) -> windows_core::Result<Visual> {
                $crate::object::not_implemented()
            }
            fn SetParentForTransform(&self, _value: Ref<Visual>) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn RelativeOffsetAdjustment(&self) -> windows_core::Result<Vector3> {
                $crate::object::not_implemented()
            }
            fn SetRelativeOffsetAdjustment(&self, _value: &Vector3) -> windows_core::Result<()> {
                $crate::object::not_implemented()
            }
            fn RelativeSizeAdjustment(&self) -> windows_core::Result<Vector2> {
                $crate::object::not_implemented()
            }
            fn SetRelativeSizeAdjustment(&self, _value: &Vector2) -> windows_core::Result<()> {
                $crate::object::not_implemented()
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

pub(crate) use composition_object;
pub(crate) use visual_trait;
