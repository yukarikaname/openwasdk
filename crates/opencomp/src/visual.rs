use crate::bindings::Microsoft::UI::Composition::{
    CompositionBackfaceVisibility, CompositionBorderMode, CompositionClip,
    CompositionCompositeMode, ContainerVisual, IContainerVisual, IContainerVisual_Impl, IVisual,
    IVisual_Impl, VisualCollection,
};
use windows_core::{Error, HRESULT, Interface, Result};

const E_NOTIMPL: HRESULT = HRESULT(0x8000_4001u32 as i32);

fn not_implemented<T>() -> Result<T> {
    Err(Error::from_hresult(E_NOTIMPL))
}

#[windows_core::implement(IContainerVisual, IVisual)]
pub struct ContainerVisualObject;

pub fn create() -> Result<super::bindings::Microsoft::UI::Composition::ContainerVisual> {
    let object: IContainerVisual = ContainerVisualObject.into();
    object.cast()
}

#[allow(unused_variables)]
impl IContainerVisual_Impl for ContainerVisualObject_Impl {
    fn Children(&self) -> windows_core::Result<VisualCollection> {
        not_implemented()
    }
}

#[allow(unused_variables)]
impl IVisual_Impl for ContainerVisualObject_Impl {
    fn AnchorPoint(&self) -> windows_core::Result<windows_numerics::Vector2> {
        not_implemented()
    }
    fn SetAnchorPoint(&self, value: &windows_numerics::Vector2) -> windows_core::Result<()> {
        not_implemented()
    }
    fn BackfaceVisibility(&self) -> windows_core::Result<CompositionBackfaceVisibility> {
        not_implemented()
    }
    fn SetBackfaceVisibility(
        &self,
        value: CompositionBackfaceVisibility,
    ) -> windows_core::Result<()> {
        not_implemented()
    }
    fn BorderMode(&self) -> windows_core::Result<CompositionBorderMode> {
        not_implemented()
    }
    fn SetBorderMode(&self, value: CompositionBorderMode) -> windows_core::Result<()> {
        not_implemented()
    }
    fn CenterPoint(&self) -> windows_core::Result<windows_numerics::Vector3> {
        not_implemented()
    }
    fn SetCenterPoint(&self, value: &windows_numerics::Vector3) -> windows_core::Result<()> {
        not_implemented()
    }
    fn Clip(&self) -> windows_core::Result<CompositionClip> {
        not_implemented()
    }
    fn SetClip(&self, value: windows_core::Ref<CompositionClip>) -> windows_core::Result<()> {
        not_implemented()
    }
    fn CompositeMode(&self) -> windows_core::Result<CompositionCompositeMode> {
        not_implemented()
    }
    fn SetCompositeMode(&self, value: CompositionCompositeMode) -> windows_core::Result<()> {
        not_implemented()
    }
    fn IsVisible(&self) -> windows_core::Result<bool> {
        not_implemented()
    }
    fn SetIsVisible(&self, value: bool) -> windows_core::Result<()> {
        not_implemented()
    }
    fn Offset(&self) -> windows_core::Result<windows_numerics::Vector3> {
        not_implemented()
    }
    fn SetOffset(&self, value: &windows_numerics::Vector3) -> windows_core::Result<()> {
        not_implemented()
    }
    fn Opacity(&self) -> windows_core::Result<f32> {
        not_implemented()
    }
    fn SetOpacity(&self, value: f32) -> windows_core::Result<()> {
        not_implemented()
    }
    fn Orientation(
        &self,
    ) -> windows_core::Result<crate::bindings::Windows::Foundation::Numerics::Quaternion> {
        not_implemented()
    }
    fn SetOrientation(
        &self,
        value: &crate::bindings::Windows::Foundation::Numerics::Quaternion,
    ) -> windows_core::Result<()> {
        not_implemented()
    }
    fn Parent(&self) -> windows_core::Result<ContainerVisual> {
        not_implemented()
    }
    fn RotationAngle(&self) -> windows_core::Result<f32> {
        not_implemented()
    }
    fn SetRotationAngle(&self, value: f32) -> windows_core::Result<()> {
        not_implemented()
    }
    fn RotationAngleInDegrees(&self) -> windows_core::Result<f32> {
        not_implemented()
    }
    fn SetRotationAngleInDegrees(&self, value: f32) -> windows_core::Result<()> {
        not_implemented()
    }
    fn RotationAxis(&self) -> windows_core::Result<windows_numerics::Vector3> {
        not_implemented()
    }
    fn SetRotationAxis(&self, value: &windows_numerics::Vector3) -> windows_core::Result<()> {
        not_implemented()
    }
    fn Scale(&self) -> windows_core::Result<windows_numerics::Vector3> {
        not_implemented()
    }
    fn SetScale(&self, value: &windows_numerics::Vector3) -> windows_core::Result<()> {
        not_implemented()
    }
    fn Size(&self) -> windows_core::Result<windows_numerics::Vector2> {
        not_implemented()
    }
    fn SetSize(&self, value: &windows_numerics::Vector2) -> windows_core::Result<()> {
        not_implemented()
    }
    fn TransformMatrix(&self) -> windows_core::Result<windows_numerics::Matrix4x4> {
        not_implemented()
    }
    fn SetTransformMatrix(&self, value: &windows_numerics::Matrix4x4) -> windows_core::Result<()> {
        not_implemented()
    }
}
