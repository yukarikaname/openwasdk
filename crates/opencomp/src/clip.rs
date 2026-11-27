//! `InsetClip`, the clip the layout system uses: a rectangle derived from the visual's own size.
//!
//! The insets are copied into the visual when the clip is attached, the same shortcut the brush
//! takes. A clip whose insets are all zero clips nothing, which is what a client gets before it
//! sets them.

use std::sync::Mutex;

use crate::bindings::Microsoft::UI::Composition::{
    CompositionClip, ICompositionClip, ICompositionClip_Impl, ICompositionClip2,
    ICompositionClip2_Impl, ICompositionObject, ICompositionObject2, ICompositionObject4,
    ICompositionObject5, IInsetClip, IInsetClip_Impl, InsetClip,
};
use crate::object::prelude::*;
use crate::object::{composition_object, not_implemented};
use windows_core::{Interface, Result};

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionClip,
    ICompositionClip2,
    IInsetClip
)]
pub struct InsetClipObject {
    /// Left, top, right, bottom.
    insets: Mutex<[f32; 4]>,
}

composition_object!(InsetClipObject_Impl);

impl ICompositionClip_Impl for InsetClipObject_Impl {}

impl ICompositionClip2_Impl for InsetClipObject_Impl {
    fn AnchorPoint(&self) -> Result<Vector2> {
        not_implemented()
    }
    fn SetAnchorPoint(&self, _value: &Vector2) -> Result<()> {
        not_implemented()
    }
    fn CenterPoint(&self) -> Result<Vector2> {
        not_implemented()
    }
    fn SetCenterPoint(&self, _value: &Vector2) -> Result<()> {
        not_implemented()
    }
    fn Offset(&self) -> Result<Vector2> {
        not_implemented()
    }
    fn SetOffset(&self, _value: &Vector2) -> Result<()> {
        not_implemented()
    }
    fn RotationAngle(&self) -> Result<f32> {
        not_implemented()
    }
    fn SetRotationAngle(&self, _value: f32) -> Result<()> {
        not_implemented()
    }
    fn RotationAngleInDegrees(&self) -> Result<f32> {
        not_implemented()
    }
    fn SetRotationAngleInDegrees(&self, _value: f32) -> Result<()> {
        not_implemented()
    }
    fn Scale(&self) -> Result<Vector2> {
        not_implemented()
    }
    fn SetScale(&self, _value: &Vector2) -> Result<()> {
        not_implemented()
    }
    fn TransformMatrix(&self) -> Result<windows_numerics::Matrix3x2> {
        not_implemented()
    }
    fn SetTransformMatrix(&self, _value: &windows_numerics::Matrix3x2) -> Result<()> {
        not_implemented()
    }
}

impl IInsetClip_Impl for InsetClipObject_Impl {
    fn LeftInset(&self) -> Result<f32> {
        Ok(self.insets.lock().unwrap()[0])
    }

    fn SetLeftInset(&self, value: f32) -> Result<()> {
        self.insets.lock().unwrap()[0] = value;
        Ok(())
    }

    fn TopInset(&self) -> Result<f32> {
        Ok(self.insets.lock().unwrap()[1])
    }

    fn SetTopInset(&self, value: f32) -> Result<()> {
        self.insets.lock().unwrap()[1] = value;
        Ok(())
    }

    fn RightInset(&self) -> Result<f32> {
        Ok(self.insets.lock().unwrap()[2])
    }

    fn SetRightInset(&self, value: f32) -> Result<()> {
        self.insets.lock().unwrap()[2] = value;
        Ok(())
    }

    fn BottomInset(&self) -> Result<f32> {
        Ok(self.insets.lock().unwrap()[3])
    }

    fn SetBottomInset(&self, value: f32) -> Result<()> {
        self.insets.lock().unwrap()[3] = value;
        Ok(())
    }
}

pub fn create() -> Result<InsetClip> {
    create_with([0.0; 4])
}

pub fn create_with(insets: [f32; 4]) -> Result<InsetClip> {
    let clip: IInsetClip = InsetClipObject {
        insets: Mutex::new(insets),
    }
    .into();
    clip.cast()
}

/// The insets of any clip object this DLL made, in left, top, right, bottom order.
pub fn insets(clip: &CompositionClip) -> Option<[f32; 4]> {
    let clip = clip.cast::<IInsetClip>().ok()?;
    Some([
        clip.LeftInset().ok()?,
        clip.TopInset().ok()?,
        clip.RightInset().ok()?,
        clip.BottomInset().ok()?,
    ])
}
