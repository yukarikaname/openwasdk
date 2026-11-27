//! The Compositor class, the root of a composition tree.
//!
//! XAML creates one of these per window and asks it for visuals, brushes and animations. Every
//! constructor is generated from the metadata and currently reports `E_NOTIMPL`; the compositor
//! grows one method at a time behind the same ABI.

use std::sync::{Arc, Mutex, OnceLock};

use crate::bindings::Microsoft::UI::Composition::*;
use crate::clock::Clock;
use crate::interop::ICompositorInterop;
use windows_core::{Error, HRESULT, Interface, Result};

const E_NOTIMPL: HRESULT = HRESULT(0x8000_4001u32 as i32);

pub(crate) fn not_implemented<T>() -> Result<T> {
    Err(Error::from_hresult(E_NOTIMPL))
}

/// Every compositor this DLL made that is still alive. `ICompositionObject.Compositor`
/// has to hand one back, and a visual only carries the clock, not the object that
/// created it; the table finds the live compositor with the same clock.
static LIVE: OnceLock<Mutex<Vec<windows_core::Weak<ICompositor>>>> = OnceLock::new();

fn live() -> &'static Mutex<Vec<windows_core::Weak<ICompositor>>> {
    LIVE.get_or_init(|| Mutex::new(Vec::new()))
}

/// `ICompositor` as an object the runtime can hand out.
#[windows_core::implement(
    ICompositor,
    ICompositor2,
    ICompositor4,
    ICompositor5,
    ICompositor6,
    ICompositor8,
    ICompositorInterop,
    ICompositorWithProjectedShadow,
    ICompositorWithRadialGradient,
    ICompositorWithVisualSurface
)]
pub struct Compositor {
    pub(crate) state: Arc<State>,
}

/// The state the compositor itself keeps. The clock is the compositor's; every visual
/// and animation it creates samples that one, and the frame loop of the bridge that
/// was created from this compositor advances it.
pub struct State {
    pub clock: Arc<Clock>,
    pub comment: Mutex<String>,
    pub playback_rate: Mutex<f32>,
}

impl State {
    fn new(clock: Arc<Clock>) -> Arc<Self> {
        Arc::new(Self {
            clock,
            comment: Mutex::new(String::new()),
            playback_rate: Mutex::new(1.0),
        })
    }
}

fn create(clock: Arc<Clock>) -> Result<ICompositor> {
    let state = State::new(clock);
    let compositor: ICompositor = Compositor {
        state: state.clone(),
    }
    .into();
    // The identity table is how the clock is found from the object a client passes
    // back, whether that object is the compositor or something it made.
    crate::identity::publish(&compositor, &state);
    if let Ok(weak) = compositor.downgrade() {
        live().lock().unwrap().push(weak);
    }
    Ok(compositor)
}

/// The object as `IInspectable`, which is what `IActivationFactory::ActivateInstance` returns.
pub fn instance() -> Result<windows_core::IInspectable> {
    create(Clock::new())?.cast()
}

/// A compositor bound to `clock`, preferring the live one if it still exists.
pub fn for_clock(clock: &Arc<Clock>) -> Result<ICompositor> {
    {
        let mut table = live().lock().unwrap();
        table.retain(|weak| weak.upgrade().is_some());
        for weak in table.iter() {
            let Some(compositor) = weak.upgrade() else {
                continue;
            };
            let Some(state) = crate::identity::lookup::<_, State>(&compositor) else {
                continue;
            };
            if Arc::ptr_eq(&state.clock, clock) {
                return Ok(compositor);
            }
        }
    }
    create(clock.clone())
}

/// The clock of a compositor this DLL made.
pub fn clock_of<T: Interface>(object: &T) -> Option<Arc<Clock>> {
    crate::identity::lookup::<_, State>(object).map(|state| state.clock.clone())
}

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
        crate::visual::container(self.state.clock.clone())
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
        _graphicsEffect: windows_core::Ref<
            crate::bindings::Windows::Graphics::Effects::IGraphicsEffect,
        >,
    ) -> windows_core::Result<CompositionEffectFactory> {
        not_implemented()
    }
    fn CreateEffectFactoryWithProperties(
        &self,
        _graphicsEffect: windows_core::Ref<
            crate::bindings::Windows::Graphics::Effects::IGraphicsEffect,
        >,
        _animatableProperties: windows_core::Ref<
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
        _expression: &windows_core::HSTRING,
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
        crate::batch::scoped(self.state.clock.clone(), batchType)
    }
    fn CreateSpriteVisual(&self) -> windows_core::Result<SpriteVisual> {
        crate::visual::sprite(self.state.clock.clone())
    }
    fn CreateSurfaceBrush(&self) -> windows_core::Result<CompositionSurfaceBrush> {
        crate::brush::surface_brush()
    }
    fn CreateSurfaceBrushWithSurface(
        &self,
        surface: windows_core::Ref<ICompositionSurface>,
    ) -> windows_core::Result<CompositionSurfaceBrush> {
        crate::brush::surface_brush_with(surface.cloned())
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
        crate::batch::commit(self.state.clock.clone(), batchType)
    }
}
