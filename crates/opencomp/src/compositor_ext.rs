//! The rest of `ICompositor`: everything a client can ask the compositor for that
//! this DLL does not build yet.
//!
//! Each follow-on interface is implemented so that `QueryInterface` succeeds. A
//! failed QI throws at the call site; a factory that returns `E_NOTIMPL` only costs
//! the feature, and the client keeps the tree it already has.
//!
//! `Comment` and `GlobalPlaybackRate` keep their meaning: a comment is a string a
//! debugger can read and the playback rate is a hint, so both answer instead of
//! failing.

use crate::bindings::Microsoft::UI::Composition as comp;
use crate::object::not_implemented;
use windows_core::HSTRING;

impl comp::ICompositor2_Impl for crate::compositor::Compositor_Impl {
    fn CreateAmbientLight(&self) -> windows_core::Result<comp::AmbientLight> {
        not_implemented()
    }
    fn CreateAnimationGroup(&self) -> windows_core::Result<comp::CompositionAnimationGroup> {
        not_implemented()
    }
    fn CreateBackdropBrush(&self) -> windows_core::Result<comp::CompositionBackdropBrush> {
        not_implemented()
    }
    fn CreateDistantLight(&self) -> windows_core::Result<comp::DistantLight> {
        not_implemented()
    }
    fn CreateDropShadow(&self) -> windows_core::Result<comp::DropShadow> {
        not_implemented()
    }
    fn CreateImplicitAnimationCollection(
        &self,
    ) -> windows_core::Result<comp::ImplicitAnimationCollection> {
        not_implemented()
    }
    fn CreateLayerVisual(&self) -> windows_core::Result<comp::LayerVisual> {
        not_implemented()
    }
    fn CreateMaskBrush(&self) -> windows_core::Result<comp::CompositionMaskBrush> {
        not_implemented()
    }
    fn CreateNineGridBrush(&self) -> windows_core::Result<comp::CompositionNineGridBrush> {
        not_implemented()
    }
    fn CreatePointLight(&self) -> windows_core::Result<comp::PointLight> {
        not_implemented()
    }
    fn CreateSpotLight(&self) -> windows_core::Result<comp::SpotLight> {
        not_implemented()
    }
    fn CreateStepEasingFunction(&self) -> windows_core::Result<comp::StepEasingFunction> {
        not_implemented()
    }
    fn CreateStepEasingFunctionWithStepCount(
        &self,
        _stepCount: i32,
    ) -> windows_core::Result<comp::StepEasingFunction> {
        not_implemented()
    }
}

impl comp::ICompositor4_Impl for crate::compositor::Compositor_Impl {
    fn CreateColorGradientStop(&self) -> windows_core::Result<comp::CompositionColorGradientStop> {
        not_implemented()
    }
    fn CreateColorGradientStopWithOffsetAndColor(
        &self,
        _offset: f32,
        _color: &crate::bindings::Windows::UI::Color,
    ) -> windows_core::Result<comp::CompositionColorGradientStop> {
        not_implemented()
    }
    fn CreateLinearGradientBrush(
        &self,
    ) -> windows_core::Result<comp::CompositionLinearGradientBrush> {
        not_implemented()
    }
    fn CreateSpringScalarAnimation(
        &self,
    ) -> windows_core::Result<comp::SpringScalarNaturalMotionAnimation> {
        not_implemented()
    }
    fn CreateSpringVector2Animation(
        &self,
    ) -> windows_core::Result<comp::SpringVector2NaturalMotionAnimation> {
        not_implemented()
    }
    fn CreateSpringVector3Animation(
        &self,
    ) -> windows_core::Result<comp::SpringVector3NaturalMotionAnimation> {
        not_implemented()
    }
}

impl comp::ICompositor5_Impl for crate::compositor::Compositor_Impl {
    fn Comment(&self) -> windows_core::Result<windows_core::HSTRING> {
        Ok(HSTRING::new())
    }
    fn SetComment(&self, _value: &windows_core::HSTRING) -> windows_core::Result<()> {
        Ok(())
    }
    fn GlobalPlaybackRate(&self) -> windows_core::Result<f32> {
        Ok(1.0)
    }
    fn SetGlobalPlaybackRate(&self, _value: f32) -> windows_core::Result<()> {
        Ok(())
    }
    fn CreateBounceScalarAnimation(
        &self,
    ) -> windows_core::Result<comp::BounceScalarNaturalMotionAnimation> {
        not_implemented()
    }
    fn CreateBounceVector2Animation(
        &self,
    ) -> windows_core::Result<comp::BounceVector2NaturalMotionAnimation> {
        not_implemented()
    }
    fn CreateBounceVector3Animation(
        &self,
    ) -> windows_core::Result<comp::BounceVector3NaturalMotionAnimation> {
        not_implemented()
    }
    fn CreateContainerShape(&self) -> windows_core::Result<comp::CompositionContainerShape> {
        not_implemented()
    }
    fn CreateEllipseGeometry(&self) -> windows_core::Result<comp::CompositionEllipseGeometry> {
        not_implemented()
    }
    fn CreateLineGeometry(&self) -> windows_core::Result<comp::CompositionLineGeometry> {
        not_implemented()
    }
    fn CreatePathGeometry(&self) -> windows_core::Result<comp::CompositionPathGeometry> {
        not_implemented()
    }
    fn CreatePathGeometryWithPath(
        &self,
        _path: windows_core::Ref<comp::CompositionPath>,
    ) -> windows_core::Result<comp::CompositionPathGeometry> {
        not_implemented()
    }
    fn CreatePathKeyFrameAnimation(&self) -> windows_core::Result<comp::PathKeyFrameAnimation> {
        not_implemented()
    }
    fn CreateRectangleGeometry(&self) -> windows_core::Result<comp::CompositionRectangleGeometry> {
        not_implemented()
    }
    fn CreateRoundedRectangleGeometry(
        &self,
    ) -> windows_core::Result<comp::CompositionRoundedRectangleGeometry> {
        not_implemented()
    }
    fn CreateShapeVisual(&self) -> windows_core::Result<comp::ShapeVisual> {
        not_implemented()
    }
    fn CreateSpriteShape(&self) -> windows_core::Result<comp::CompositionSpriteShape> {
        not_implemented()
    }
    fn CreateSpriteShapeWithGeometry(
        &self,
        _geometry: windows_core::Ref<comp::CompositionGeometry>,
    ) -> windows_core::Result<comp::CompositionSpriteShape> {
        not_implemented()
    }
    fn CreateViewBox(&self) -> windows_core::Result<comp::CompositionViewBox> {
        not_implemented()
    }
    fn RequestCommitAsync(&self) -> windows_core::Result<windows_future::IAsyncAction> {
        not_implemented()
    }
}

impl comp::ICompositor6_Impl for crate::compositor::Compositor_Impl {
    fn CreateGeometricClip(&self) -> windows_core::Result<comp::CompositionGeometricClip> {
        not_implemented()
    }
    fn CreateGeometricClipWithGeometry(
        &self,
        _geometry: windows_core::Ref<comp::CompositionGeometry>,
    ) -> windows_core::Result<comp::CompositionGeometricClip> {
        not_implemented()
    }
    fn CreateRedirectVisual(&self) -> windows_core::Result<comp::RedirectVisual> {
        not_implemented()
    }
    fn CreateRedirectVisualWithSourceVisual(
        &self,
        _source: windows_core::Ref<comp::Visual>,
    ) -> windows_core::Result<comp::RedirectVisual> {
        not_implemented()
    }
    fn CreateBooleanKeyFrameAnimation(
        &self,
    ) -> windows_core::Result<comp::BooleanKeyFrameAnimation> {
        not_implemented()
    }
}

impl comp::ICompositor8_Impl for crate::compositor::Compositor_Impl {
    fn CreateAnimationController(&self) -> windows_core::Result<comp::AnimationController> {
        not_implemented()
    }
}

impl comp::ICompositorWithProjectedShadow_Impl for crate::compositor::Compositor_Impl {
    fn CreateProjectedShadowCaster(
        &self,
    ) -> windows_core::Result<comp::CompositionProjectedShadowCaster> {
        not_implemented()
    }
    fn CreateProjectedShadow(&self) -> windows_core::Result<comp::CompositionProjectedShadow> {
        not_implemented()
    }
    fn CreateProjectedShadowReceiver(
        &self,
    ) -> windows_core::Result<comp::CompositionProjectedShadowReceiver> {
        not_implemented()
    }
}

impl comp::ICompositorWithRadialGradient_Impl for crate::compositor::Compositor_Impl {
    fn CreateRadialGradientBrush(
        &self,
    ) -> windows_core::Result<comp::CompositionRadialGradientBrush> {
        not_implemented()
    }
}

impl comp::ICompositorWithVisualSurface_Impl for crate::compositor::Compositor_Impl {
    fn CreateVisualSurface(&self) -> windows_core::Result<comp::CompositionVisualSurface> {
        not_implemented()
    }
}
