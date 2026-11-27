//! The Compositor class, the root of a composition tree.
//!
//! XAML creates one of these per window and asks it for visuals, brushes and animations. Every
//! constructor is generated from the metadata and currently reports `E_NOTIMPL`; the compositor
//! grows one method at a time behind the same ABI.

use crate::bindings::Microsoft::UI::Composition::*;
use windows_core::{Error, HRESULT, Interface, Result};

const E_NOTIMPL: HRESULT = HRESULT(0x8000_4001u32 as i32);

fn not_implemented<T>() -> Result<T> {
    Err(Error::from_hresult(E_NOTIMPL))
}

/// `ICompositor` as an object the runtime can hand out.
#[windows_core::implement(ICompositor)]
pub struct Compositor;

/// The object as `IInspectable`, which is what `IActivationFactory::ActivateInstance` returns.
pub fn instance() -> Result<windows_core::IInspectable> {
    let compositor: ICompositor = Compositor.into();
    compositor.cast()
}

#[allow(unused_variables)]
impl ICompositor_Impl for Compositor_Impl {
    fn CreateColorKeyFrameAnimation(&self) -> windows_core::Result<ColorKeyFrameAnimation> {
        crate::color_animation::color()
    }
    fn CreateColorBrush(&self) -> windows_core::Result<CompositionColorBrush> {
        crate::brush::create()
    }
    fn CreateColorBrushWithColor(
        &self,
        color: &crate::bindings::Windows::UI::Color,
    ) -> windows_core::Result<CompositionColorBrush> {
        crate::brush::create_with(*color)
    }
    fn CreateContainerVisual(&self) -> windows_core::Result<ContainerVisual> {
        crate::visual::container()
    }
    fn CreateCubicBezierEasingFunction(
        &self,
        controlPoint1: &windows_numerics::Vector2,
        controlPoint2: &windows_numerics::Vector2,
    ) -> windows_core::Result<CubicBezierEasingFunction> {
        crate::easing::cubic_bezier(*controlPoint1, *controlPoint2)
    }
    fn CreateEffectFactory(
        &self,
        graphicsEffect: windows_core::Ref<
            crate::bindings::Windows::Graphics::Effects::IGraphicsEffect,
        >,
    ) -> windows_core::Result<CompositionEffectFactory> {
        not_implemented()
    }
    fn CreateEffectFactoryWithProperties(
        &self,
        graphicsEffect: windows_core::Ref<
            crate::bindings::Windows::Graphics::Effects::IGraphicsEffect,
        >,
        animatableProperties: windows_core::Ref<
            windows_collections::IIterable<windows_core::HSTRING>,
        >,
    ) -> windows_core::Result<CompositionEffectFactory> {
        not_implemented()
    }
    fn CreateExpressionAnimation(&self) -> windows_core::Result<ExpressionAnimation> {
        not_implemented()
    }
    fn CreateExpressionAnimationWithExpression(
        &self,
        expression: &windows_core::HSTRING,
    ) -> windows_core::Result<ExpressionAnimation> {
        not_implemented()
    }
    fn CreateInsetClip(&self) -> windows_core::Result<InsetClip> {
        crate::clip::create()
    }
    fn CreateInsetClipWithInsets(
        &self,
        leftInset: f32,
        topInset: f32,
        rightInset: f32,
        bottomInset: f32,
    ) -> windows_core::Result<InsetClip> {
        crate::clip::create_with([leftInset, topInset, rightInset, bottomInset])
    }
    fn CreateLinearEasingFunction(&self) -> windows_core::Result<LinearEasingFunction> {
        crate::easing::linear()
    }
    fn CreatePropertySet(&self) -> windows_core::Result<CompositionPropertySet> {
        crate::propertyset::create()
    }
    fn CreateQuaternionKeyFrameAnimation(
        &self,
    ) -> windows_core::Result<QuaternionKeyFrameAnimation> {
        not_implemented()
    }
    fn CreateScalarKeyFrameAnimation(&self) -> windows_core::Result<ScalarKeyFrameAnimation> {
        crate::animation::scalar()
    }
    fn CreateScopedBatch(
        &self,
        batchType: CompositionBatchTypes,
    ) -> windows_core::Result<CompositionScopedBatch> {
        not_implemented()
    }
    fn CreateSpriteVisual(&self) -> windows_core::Result<SpriteVisual> {
        crate::visual::sprite()
    }
    fn CreateSurfaceBrush(&self) -> windows_core::Result<CompositionSurfaceBrush> {
        not_implemented()
    }
    fn CreateSurfaceBrushWithSurface(
        &self,
        surface: windows_core::Ref<ICompositionSurface>,
    ) -> windows_core::Result<CompositionSurfaceBrush> {
        not_implemented()
    }
    fn CreateVector2KeyFrameAnimation(&self) -> windows_core::Result<Vector2KeyFrameAnimation> {
        crate::animation::vector2()
    }
    fn CreateVector3KeyFrameAnimation(&self) -> windows_core::Result<Vector3KeyFrameAnimation> {
        crate::animation::vector3()
    }
    fn CreateVector4KeyFrameAnimation(&self) -> windows_core::Result<Vector4KeyFrameAnimation> {
        not_implemented()
    }
    fn GetCommitBatch(
        &self,
        batchType: CompositionBatchTypes,
    ) -> windows_core::Result<CompositionCommitBatch> {
        not_implemented()
    }
}
