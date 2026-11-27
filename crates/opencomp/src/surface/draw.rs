//! The interop half of a drawing surface: `BeginDraw` hands out the Direct2D context.
//!
//! A client asks for `ID2D1DeviceContext` and draws through it; this module points the shared
//! context at the surface's own texture and adds a reference for the caller. The client then
//! calls the context's own `BeginDraw`/`EndDraw` for its drawing commands, and finishes with
//! this interface's `EndDraw`, which copies the texture back into the CPU buffer observed by
//! the composition path.
//!
//! The update rectangle is not honoured: the context targets the whole surface, so the
//! returned offset is always `(0, 0)` and the client draws in surface coordinates. Scrolling
//! is the only caller that would need the offset, and it is not implemented.
//!
//! A surface made without a graphics device, or over an object that is not a D3D11 device,
//! has no context to give and keeps answering `E_NOINTERFACE`, which is what the CPU-only
//! path did before.

use std::ffi::c_void;
use std::sync::atomic::Ordering;

use windows_core::{Error, GUID, HRESULT, Result};

use crate::interop::{
    ICompositionDrawingSurfaceInterop_Impl, ID2D1_DEVICE_CONTEXT, PixelSize, Rect,
};
use crate::object::not_implemented;

use super::{DrawingSurfaceObject_Impl, E_OUTOFMEMORY};

const E_NOINTERFACE: HRESULT = HRESULT(0x8000_4002u32 as i32);
const E_FAIL: HRESULT = HRESULT(0x8000_4005u32 as i32);

impl ICompositionDrawingSurfaceInterop_Impl for DrawingSurfaceObject_Impl {
    fn BeginDraw(&self, _updateRect: *const Rect, iid: &GUID) -> Result<*mut c_void> {
        if iid != &ID2D1_DEVICE_CONTEXT {
            return Err(Error::from_hresult(E_NOINTERFACE));
        }
        // Without a graphics device there is no Direct2D at all: the CPU path answers the
        // same way it always has.
        let Some(gpu) = self.surface.gpu.as_ref() else {
            return Err(Error::from_hresult(E_NOINTERFACE));
        };
        // One session per surface; a second BeginDraw before EndDraw is a client bug.
        if self.surface.drawing.swap(true, Ordering::Acquire) {
            return Err(Error::from_hresult(E_FAIL));
        }
        let started = match self.surface.texture.lock() {
            Ok(texture) => texture.as_ref().and_then(|texture| gpu.begin(texture)),
            Err(_) => None,
        };
        match started {
            Some(context) => Ok(context),
            None => {
                self.surface.drawing.store(false, Ordering::Release);
                Err(Error::from_hresult(E_FAIL))
            }
        }
    }

    fn EndDraw(&self) -> Result<()> {
        if !self.surface.drawing.swap(false, Ordering::AcqRel) {
            return Err(Error::from_hresult(E_FAIL));
        }
        let Some(gpu) = self.surface.gpu.as_ref() else {
            // A session only opens with a device, but a surface cannot stay wedged if it
            // somehow got here.
            return Err(Error::from_hresult(E_FAIL));
        };
        // A missing texture still has to close the context, so pass the `None` through.
        let pixels = match self.surface.texture.lock() {
            Ok(texture) => gpu.end(texture.as_ref()),
            Err(_) => gpu.end(None),
        };
        let Some(pixels) = pixels else {
            return Err(Error::from_hresult(E_FAIL));
        };
        // The composition path samples the CPU buffer, so it has to see what was drawn.
        let Ok(mut buffer) = self.surface.pixels.lock() else {
            return Err(Error::from_hresult(E_FAIL));
        };
        if buffer.rgba.len() != pixels.len() {
            return Err(Error::from_hresult(E_FAIL));
        }
        buffer.rgba.copy_from_slice(&pixels);
        Ok(())
    }

    fn Resize(&self, sizePixels: PixelSize) -> Result<()> {
        let width = sizePixels.cx.max(0) as u32;
        let height = sizePixels.cy.max(0) as u32;
        if self.surface.resize(width, height) {
            Ok(())
        } else {
            Err(Error::from_hresult(E_OUTOFMEMORY))
        }
    }

    // Scrolling would have to move pixels the host may be filling; it is not part of this path.
    fn Scroll(
        &self,
        _scrollRect: *const Rect,
        _clipRect: *const Rect,
        _offsetX: i32,
        _offsetY: i32,
    ) -> Result<()> {
        not_implemented()
    }

    fn ResumeDraw(&self) -> Result<()> {
        not_implemented()
    }

    fn SuspendDraw(&self) -> Result<()> {
        not_implemented()
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::bindings::Microsoft::Graphics::DirectX::{DirectXAlphaMode, DirectXPixelFormat};
    use crate::bindings::Microsoft::UI::Composition::{
        CompositionDrawingSurface, ICompositionGraphicsDevice, ICompositor,
    };
    use crate::bindings::Windows::Foundation::Size;
    use crate::interop::{
        ICompositionDrawingSurfaceInterop, ICompositorInterop, ID2D1_DEVICE_CONTEXT,
    };
    use crate::surface::surface_pixels;
    use windows::Win32::Foundation::HMODULE;
    use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_COLOR_F};
    use windows::Win32::Graphics::Direct2D::ID2D1DeviceContext;
    use windows::Win32::Graphics::Direct3D::{
        D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_10_1,
        D3D_FEATURE_LEVEL_11_0,
    };
    use windows::Win32::Graphics::Direct3D11::{
        D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
    };
    use windows_core::{IUnknown, Interface};

    /// A hardware device, as the compositor makes one, or `None` under a prefix without one.
    fn d3d11() -> Option<ID3D11Device> {
        let mut device = None;
        let mut context = None;
        let levels = [
            D3D_FEATURE_LEVEL_11_0,
            D3D_FEATURE_LEVEL_10_1,
            D3D_FEATURE_LEVEL_10_0,
        ];
        // SAFETY: the out parameters are ours and only read after the call succeeds.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        }
        .ok()?;
        device
    }

    /// The same device as the `windows-core` 0.100 `IUnknown` the interop layer stores.
    fn unknown(device: &ID3D11Device) -> IUnknown {
        // The clone adds a reference and `into_raw` hands it over; `from_raw` takes it.
        let raw = windows::core::Interface::into_raw(device.clone());
        // SAFETY: both crates describe the same COM base vtable and `raw` owns a reference.
        unsafe { windows_core::Interface::from_raw(raw) }
    }

    /// A surface the way XAML makes one: through a graphics device over a real D3D11 device.
    fn surface() -> Option<(CompositionDrawingSurface, ICompositionDrawingSurfaceInterop)> {
        let device = d3d11()?;
        let compositor: ICompositor = crate::compositor::instance().unwrap().cast().unwrap();
        let factory: ICompositorInterop = compositor.cast().unwrap();
        let graphics: ICompositionGraphicsDevice =
            factory.CreateGraphicsDevice(&unknown(&device)).unwrap();
        let surface = graphics
            .CreateDrawingSurface(
                Size {
                    width: 4.0,
                    height: 4.0,
                },
                DirectXPixelFormat::R8G8B8A8UIntNormalizedSrgb,
                DirectXAlphaMode::Premultiplied,
            )
            .unwrap();
        let interop: ICompositionDrawingSurfaceInterop = surface.cast().unwrap();
        // A machine without Direct2D leaves the surface on the CPU path; nothing to test.
        let probe = interop.BeginDraw(&ID2D1_DEVICE_CONTEXT).ok()?;
        let probe: ID2D1DeviceContext = unsafe { windows::core::Interface::from_raw(probe) };
        drop(probe);
        interop.EndDraw().ok()?;
        Some((surface, interop))
    }

    #[test]
    fn begin_draw_returns_a_d2d_device_context() {
        let Some((_surface, interop)) = surface() else {
            return;
        };
        let context = interop.BeginDraw(&ID2D1_DEVICE_CONTEXT).expect("context");
        // Take ownership of the reference BeginDraw added and check it is usable.
        let context: ID2D1DeviceContext = unsafe { windows::core::Interface::from_raw(context) };
        assert!(unsafe { context.GetTarget() }.is_ok());
        drop(context);
        interop.EndDraw().unwrap();
    }

    #[test]
    fn begin_draw_refuses_another_interface() {
        let Some((_surface, interop)) = surface() else {
            return;
        };
        let other = GUID::from_u128(0x1234_5678_9abc_def0_1234_5678_9abc_def0);
        let error = interop.BeginDraw(&other).unwrap_err();
        assert_eq!(error.code(), E_NOINTERFACE);
    }

    #[test]
    fn a_second_begin_draw_fails() {
        let Some((_surface, interop)) = surface() else {
            return;
        };
        let first = interop.BeginDraw(&ID2D1_DEVICE_CONTEXT).unwrap();
        let error = interop.BeginDraw(&ID2D1_DEVICE_CONTEXT).unwrap_err();
        assert_eq!(error.code(), E_FAIL);
        let first: ID2D1DeviceContext = unsafe { windows::core::Interface::from_raw(first) };
        drop(first);
        interop.EndDraw().unwrap();
    }

    #[test]
    fn end_draw_without_begin_draw_fails() {
        let Some((_surface, interop)) = surface() else {
            return;
        };
        assert_eq!(interop.EndDraw().unwrap_err().code(), E_FAIL);
    }

    #[test]
    fn a_filled_rectangle_reaches_the_cpu_snapshot() {
        let Some((surface, interop)) = surface() else {
            return;
        };
        let context = interop.BeginDraw(&ID2D1_DEVICE_CONTEXT).unwrap();
        let context: ID2D1DeviceContext = unsafe { windows::core::Interface::from_raw(context) };
        // SAFETY: the context targets the surface texture BeginDraw set.
        unsafe {
            context.BeginDraw();
            let brush = context
                .CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                    None,
                )
                .unwrap();
            context.FillRectangle(
                &D2D_RECT_F {
                    left: 0.0,
                    top: 0.0,
                    right: 4.0,
                    bottom: 4.0,
                },
                &brush,
            );
            context.EndDraw(None, None).unwrap();
        }
        drop(context);
        interop.EndDraw().unwrap();

        let (width, height, pixels) = surface_pixels(&surface).unwrap();
        let centre = ((height / 2 * width + width / 2) * 4) as usize;
        let pixel = &pixels[centre..centre + 4];
        assert!(pixel[0] > 240, "{pixel:?}");
        assert!(pixel[1] < 16, "{pixel:?}");
        assert!(pixel[2] < 16, "{pixel:?}");
        assert!(pixel[3] > 240, "{pixel:?}");
    }
}
