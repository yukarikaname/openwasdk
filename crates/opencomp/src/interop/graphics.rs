//! `CompositionGraphicsDevice`: the surface factory over the client's rendering device.
//!
//! The client brings its own `ID3D11Device`; this object only holds it, hands it back, and
//! creates surfaces, which is all the interop interfaces ask of it. Nothing is allocated on
//! the GPU yet, so a surface is the CPU bitmap `surface` builds.

use std::ffi::c_void;
use std::sync::Mutex;

use crate::bindings::Microsoft::Graphics::DirectX::{DirectXAlphaMode, DirectXPixelFormat};
use crate::bindings::Microsoft::UI::Composition::{
    CompositionGraphicsDevice, ICompositionGraphicsDevice, ICompositionGraphicsDevice_Impl,
    ICompositionObject, ICompositionObject2, ICompositionObject4, ICompositionObject5,
    RenderingDeviceReplacedEventArgs,
};
use crate::bindings::Windows::Foundation::Size;
use crate::bindings::Windows::Foundation::TypedEventHandler;
use crate::object::{composition_object, not_implemented};
use windows_core::{Error, IUnknown, Interface, Ref, Result};

use super::{E_POINTER, ICompositionGraphicsDeviceInterop, ICompositionGraphicsDeviceInterop_Impl};

/// The `RenderingDeviceReplaced` subscriptions.
///
/// The event never fires: this DLL wraps the client's own rendering device and never
/// replaces it. Keeping the handler lets a client subscribe and unsubscribe without seeing
/// an error while it sets the device up.
struct Events {
    next_token: i64,
    handlers: Vec<(
        i64,
        TypedEventHandler<CompositionGraphicsDevice, RenderingDeviceReplacedEventArgs>,
    )>,
}

impl Default for Events {
    fn default() -> Self {
        Self {
            // Some clients read a zero token as "not registered".
            next_token: 1,
            handlers: Vec::new(),
        }
    }
}

/// `CompositionGraphicsDevice`: the surface factory over the client's rendering device.
#[windows_core::implement(
    ICompositionGraphicsDevice,
    ICompositionGraphicsDeviceInterop,
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5
)]
pub struct GraphicsDevice {
    device: Mutex<Option<IUnknown>>,
    events: Mutex<Events>,
}

composition_object!(GraphicsDevice_Impl);

/// A graphics device over `renderingDevice`, as the compositor's interop method returns it.
pub(super) fn create(renderingDevice: IUnknown) -> ICompositionGraphicsDevice {
    GraphicsDevice {
        device: Mutex::new(Some(renderingDevice)),
        events: Mutex::new(Events::default()),
    }
    .into()
}

impl ICompositionGraphicsDevice_Impl for GraphicsDevice_Impl {
    fn CreateDrawingSurface(
        &self,
        sizePixels: &Size,
        pixelFormat: DirectXPixelFormat,
        alphaMode: DirectXAlphaMode,
    ) -> Result<crate::bindings::Microsoft::UI::Composition::CompositionDrawingSurface> {
        crate::surface::create_sized(*sizePixels, pixelFormat, alphaMode)
    }

    fn RenderingDeviceReplaced(
        &self,
        handler: Ref<
            TypedEventHandler<CompositionGraphicsDevice, RenderingDeviceReplacedEventArgs>,
        >,
    ) -> Result<i64> {
        let Some(handler) = handler.cloned() else {
            return Err(Error::from_hresult(E_POINTER));
        };
        let mut events = self.events.lock().unwrap();
        let token = events.next_token;
        events.next_token += 1;
        events.handlers.push((token, handler));
        Ok(token)
    }

    fn RemoveRenderingDeviceReplaced(&self, token: i64) -> Result<()> {
        let mut events = self.events.lock().unwrap();
        events.handlers.retain(|(existing, _)| *existing != token);
        Ok(())
    }
}

impl ICompositionGraphicsDeviceInterop_Impl for GraphicsDevice_Impl {
    fn GetRenderingDevice(&self) -> Result<IUnknown> {
        match self.device.lock().unwrap().clone() {
            Some(device) => Ok(device),
            None => not_implemented(),
        }
    }

    fn SetRenderingDevice(&self, value: Ref<IUnknown>) -> Result<()> {
        let Some(value) = value.cloned() else {
            return Err(Error::from_hresult(E_POINTER));
        };
        *self.device.lock().unwrap() = Some(value);
        Ok(())
    }
}

impl ICompositionGraphicsDeviceInterop {
    /// The device the graphics device was created over, or set to afterwards.
    pub fn GetRenderingDevice(&self) -> Result<IUnknown> {
        let mut result: *mut c_void = core::ptr::null_mut();
        unsafe {
            (Interface::vtable(self).GetRenderingDevice)(Interface::as_raw(self), &mut result)
                .ok()?;
            Ok(IUnknown::from_raw(result))
        }
    }

    /// Replace the device the graphics device renders with.
    pub fn SetRenderingDevice(&self, value: &IUnknown) -> Result<()> {
        unsafe {
            (Interface::vtable(self).SetRenderingDevice)(Interface::as_raw(self), value.as_raw())
                .ok()
        }
    }
}
