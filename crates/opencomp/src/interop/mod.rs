//! The native interoperation interfaces, which a header declares and no winmd defines.
//!
//! `Microsoft.UI.Composition.Interop.h` declares `ICompositorInterop`,
//! `ICompositionGraphicsDeviceInterop` and `ICompositionDrawingSurfaceInterop` by hand, so
//! the metadata generator never sees them and `bindings` cannot contain them. They are
//! written out here with the IIDs and the vtable order the header gives.
//!
//! This is the route a WinUI client takes to draw into one of our surfaces: the compositor
//! wraps the client's own `ID3D11Device` in a graphics device, the device creates a drawing
//! surface, and the surface is asked to begin a draw. The first two steps answer;
//! `BeginDraw` says why it has no Direct2D device context to hand out yet.

use std::ffi::c_void;

use crate::bindings::Microsoft::UI::Composition::ICompositionGraphicsDevice;
use windows_core::{Error, GUID, HRESULT, IUnknown, Interface, Ref, Result};

mod graphics;

const E_POINTER: HRESULT = HRESULT(0x8000_4003u32 as i32);

/// `ID2D1DeviceContext`, the object a client usually asks `BeginDraw` for.
pub(crate) const ID2D1_DEVICE_CONTEXT: GUID =
    GUID::from_u128(0x735d7a0a_3b74_4b90_9a4b_3cdb3f67a13f);

/// A Win32 `RECT`: the update or scroll rectangle, in pixels.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// A Win32 `SIZE`, which `Resize` passes by value.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PixelSize {
    pub cx: i32,
    pub cy: i32,
}

/// A Win32 `POINT`, where an update starts inside the surface.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
pub struct ICompositorInterop_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub CreateGraphicsDevice:
        unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> HRESULT,
}

windows_core::imp::define_interface!(
    ICompositorInterop,
    ICompositorInterop_Vtbl,
    0xfab19398_6d19_4d8a_b752_8f096c396069
);

pub trait ICompositorInterop_Impl: windows_core::IUnknownImpl {
    fn CreateGraphicsDevice(
        &self,
        renderingDevice: Ref<IUnknown>,
    ) -> Result<ICompositionGraphicsDevice>;
}

impl ICompositorInterop_Vtbl {
    pub const fn new<Identity: ICompositorInterop_Impl, const OFFSET: isize>() -> Self {
        unsafe extern "system" fn CreateGraphicsDevice<
            Identity: ICompositorInterop_Impl,
            const OFFSET: isize,
        >(
            this: *mut c_void,
            renderingDevice: *mut c_void,
            result__: *mut *mut c_void,
        ) -> HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                if renderingDevice.is_null() || result__.is_null() {
                    return E_POINTER;
                }
                match ICompositorInterop_Impl::CreateGraphicsDevice(
                    this,
                    core::mem::transmute_copy(&renderingDevice),
                ) {
                    Ok(ok__) => {
                        result__.write(core::mem::transmute_copy(&ok__));
                        core::mem::forget(ok__);
                        HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            CreateGraphicsDevice: CreateGraphicsDevice::<Identity, OFFSET>,
        }
    }

    pub fn matches(iid: &GUID) -> bool {
        iid == &<ICompositorInterop as Interface>::IID
    }
}

#[repr(C)]
pub struct ICompositionGraphicsDeviceInterop_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetRenderingDevice: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    pub SetRenderingDevice: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
}

windows_core::imp::define_interface!(
    ICompositionGraphicsDeviceInterop,
    ICompositionGraphicsDeviceInterop_Vtbl,
    0x4afa8030_bc70_4b0c_b1c7_6e69f933dc83
);

pub trait ICompositionGraphicsDeviceInterop_Impl: windows_core::IUnknownImpl {
    fn GetRenderingDevice(&self) -> Result<IUnknown>;
    fn SetRenderingDevice(&self, value: Ref<IUnknown>) -> Result<()>;
}

impl ICompositionGraphicsDeviceInterop_Vtbl {
    pub const fn new<Identity: ICompositionGraphicsDeviceInterop_Impl, const OFFSET: isize>() -> Self
    {
        unsafe extern "system" fn GetRenderingDevice<
            Identity: ICompositionGraphicsDeviceInterop_Impl,
            const OFFSET: isize,
        >(
            this: *mut c_void,
            result__: *mut *mut c_void,
        ) -> HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                if result__.is_null() {
                    return E_POINTER;
                }
                match ICompositionGraphicsDeviceInterop_Impl::GetRenderingDevice(this) {
                    Ok(ok__) => {
                        result__.write(core::mem::transmute_copy(&ok__));
                        core::mem::forget(ok__);
                        HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn SetRenderingDevice<
            Identity: ICompositionGraphicsDeviceInterop_Impl,
            const OFFSET: isize,
        >(
            this: *mut c_void,
            value: *mut c_void,
        ) -> HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ICompositionGraphicsDeviceInterop_Impl::SetRenderingDevice(
                    this,
                    core::mem::transmute_copy(&value),
                )
                .into()
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            GetRenderingDevice: GetRenderingDevice::<Identity, OFFSET>,
            SetRenderingDevice: SetRenderingDevice::<Identity, OFFSET>,
        }
    }

    pub fn matches(iid: &GUID) -> bool {
        iid == &<ICompositionGraphicsDeviceInterop as Interface>::IID
    }
}

#[repr(C)]
pub struct ICompositionDrawingSurfaceInterop_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub BeginDraw: unsafe extern "system" fn(
        *mut c_void,
        *const Rect,
        *const GUID,
        *mut *mut c_void,
        *mut Point,
    ) -> HRESULT,
    pub EndDraw: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    pub Resize: unsafe extern "system" fn(*mut c_void, PixelSize) -> HRESULT,
    pub Scroll:
        unsafe extern "system" fn(*mut c_void, *const Rect, *const Rect, i32, i32) -> HRESULT,
    pub ResumeDraw: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    pub SuspendDraw: unsafe extern "system" fn(*mut c_void) -> HRESULT,
}

windows_core::imp::define_interface!(
    ICompositionDrawingSurfaceInterop,
    ICompositionDrawingSurfaceInterop_Vtbl,
    0x2d6355c2_ad57_4eae_92e4_4c3eff65d578
);

pub trait ICompositionDrawingSurfaceInterop_Impl: windows_core::IUnknownImpl {
    /// The object `iid` names, plus where in the surface the update starts.
    fn BeginDraw(&self, updateRect: *const Rect, iid: &GUID) -> Result<*mut c_void>;
    fn EndDraw(&self) -> Result<()>;
    fn Resize(&self, sizePixels: PixelSize) -> Result<()>;
    fn Scroll(
        &self,
        scrollRect: *const Rect,
        clipRect: *const Rect,
        offsetX: i32,
        offsetY: i32,
    ) -> Result<()>;
    fn ResumeDraw(&self) -> Result<()>;
    fn SuspendDraw(&self) -> Result<()>;
}

impl ICompositionDrawingSurfaceInterop_Vtbl {
    pub const fn new<Identity: ICompositionDrawingSurfaceInterop_Impl, const OFFSET: isize>() -> Self
    {
        unsafe extern "system" fn BeginDraw<
            Identity: ICompositionDrawingSurfaceInterop_Impl,
            const OFFSET: isize,
        >(
            this: *mut c_void,
            updateRect: *const Rect,
            iid: *const GUID,
            updateObject: *mut *mut c_void,
            updateOffset: *mut Point,
        ) -> HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                if iid.is_null() || updateObject.is_null() || updateOffset.is_null() {
                    return E_POINTER;
                }
                // A failed call leaves the caller nothing to release and nothing to offset.
                *updateObject = core::ptr::null_mut();
                *updateOffset = Point::default();
                match ICompositionDrawingSurfaceInterop_Impl::BeginDraw(this, updateRect, &*iid) {
                    Ok(ok__) => {
                        *updateObject = ok__;
                        HRESULT(0)
                    }
                    Err(err) => err.into(),
                }
            }
        }
        unsafe extern "system" fn EndDraw<
            Identity: ICompositionDrawingSurfaceInterop_Impl,
            const OFFSET: isize,
        >(
            this: *mut c_void,
        ) -> HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ICompositionDrawingSurfaceInterop_Impl::EndDraw(this).into()
            }
        }
        unsafe extern "system" fn Resize<
            Identity: ICompositionDrawingSurfaceInterop_Impl,
            const OFFSET: isize,
        >(
            this: *mut c_void,
            sizePixels: PixelSize,
        ) -> HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ICompositionDrawingSurfaceInterop_Impl::Resize(this, sizePixels).into()
            }
        }
        unsafe extern "system" fn Scroll<
            Identity: ICompositionDrawingSurfaceInterop_Impl,
            const OFFSET: isize,
        >(
            this: *mut c_void,
            scrollRect: *const Rect,
            clipRect: *const Rect,
            offsetX: i32,
            offsetY: i32,
        ) -> HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ICompositionDrawingSurfaceInterop_Impl::Scroll(
                    this, scrollRect, clipRect, offsetX, offsetY,
                )
                .into()
            }
        }
        unsafe extern "system" fn ResumeDraw<
            Identity: ICompositionDrawingSurfaceInterop_Impl,
            const OFFSET: isize,
        >(
            this: *mut c_void,
        ) -> HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ICompositionDrawingSurfaceInterop_Impl::ResumeDraw(this).into()
            }
        }
        unsafe extern "system" fn SuspendDraw<
            Identity: ICompositionDrawingSurfaceInterop_Impl,
            const OFFSET: isize,
        >(
            this: *mut c_void,
        ) -> HRESULT {
            unsafe {
                let this: &Identity =
                    &*((this as *const *const ()).offset(OFFSET) as *const Identity);
                ICompositionDrawingSurfaceInterop_Impl::SuspendDraw(this).into()
            }
        }
        Self {
            base__: windows_core::IUnknown_Vtbl::new::<Identity, OFFSET>(),
            BeginDraw: BeginDraw::<Identity, OFFSET>,
            EndDraw: EndDraw::<Identity, OFFSET>,
            Resize: Resize::<Identity, OFFSET>,
            Scroll: Scroll::<Identity, OFFSET>,
            ResumeDraw: ResumeDraw::<Identity, OFFSET>,
            SuspendDraw: SuspendDraw::<Identity, OFFSET>,
        }
    }

    pub fn matches(iid: &GUID) -> bool {
        iid == &<ICompositionDrawingSurfaceInterop as Interface>::IID
    }
}

impl ICompositorInterop_Impl for crate::compositor::Compositor_Impl {
    fn CreateGraphicsDevice(
        &self,
        renderingDevice: Ref<IUnknown>,
    ) -> Result<ICompositionGraphicsDevice> {
        // The rendering device is the client's ID3D11Device; the interop interface speaks of
        // it only as IUnknown, so it is stored and handed back unchanged.
        let Some(rendering_device) = renderingDevice.cloned() else {
            return Err(Error::from_hresult(E_POINTER));
        };
        Ok(graphics::create(rendering_device))
    }
}

impl ICompositorInterop {
    /// Wrap `renderingDevice` in a graphics device, as the header's `CreateGraphicsDevice`.
    pub fn CreateGraphicsDevice(
        &self,
        renderingDevice: &IUnknown,
    ) -> Result<ICompositionGraphicsDevice> {
        let mut result: *mut c_void = core::ptr::null_mut();
        unsafe {
            (Interface::vtable(self).CreateGraphicsDevice)(
                Interface::as_raw(self),
                renderingDevice.as_raw(),
                &mut result,
            )
            .ok()?;
            Ok(ICompositionGraphicsDevice::from_raw(result))
        }
    }
}

impl ICompositionDrawingSurfaceInterop {
    /// Start a draw over the whole surface, asking for the object `iid` names.
    pub fn BeginDraw(&self, iid: &GUID) -> Result<*mut c_void> {
        let mut object: *mut c_void = core::ptr::null_mut();
        let mut offset = Point::default();
        unsafe {
            (Interface::vtable(self).BeginDraw)(
                Interface::as_raw(self),
                core::ptr::null(),
                iid,
                &mut object,
                &mut offset,
            )
            .ok()?;
        }
        Ok(object)
    }

    /// Finish the draw `BeginDraw` started.
    pub fn EndDraw(&self) -> Result<()> {
        unsafe { (Interface::vtable(self).EndDraw)(Interface::as_raw(self)).ok() }
    }

    /// Give the surface a new pixel size.
    pub fn Resize(&self, sizePixels: PixelSize) -> Result<()> {
        unsafe { (Interface::vtable(self).Resize)(Interface::as_raw(self), sizePixels).ok() }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::bindings::Microsoft::Graphics::DirectX::{DirectXAlphaMode, DirectXPixelFormat};
    use crate::bindings::Microsoft::UI::Composition::ICompositor;
    use crate::bindings::Windows::Foundation::Size;

    const E_NOINTERFACE: HRESULT = HRESULT(0x8000_4002u32 as i32);

    fn compositor() -> ICompositor {
        crate::compositor::instance().unwrap().cast().unwrap()
    }

    fn device(compositor: &ICompositor) -> ICompositionGraphicsDevice {
        let interop: ICompositorInterop = compositor.cast().unwrap();
        // Any COM object stands in for the client's ID3D11Device: the interop layer only
        // holds it as IUnknown and never calls a D3D method on it.
        let rendering: IUnknown = compositor.cast().unwrap();
        interop.CreateGraphicsDevice(&rendering).unwrap()
    }

    #[test]
    fn a_compositor_answers_the_interop_interface() {
        let compositor = compositor();
        assert!(compositor.cast::<ICompositorInterop>().is_ok());
    }

    #[test]
    fn a_graphics_device_wraps_the_clients_device() {
        let compositor = compositor();
        let rendering: IUnknown = compositor.cast().unwrap();
        let interop: ICompositorInterop = compositor.cast().unwrap();
        let device = interop.CreateGraphicsDevice(&rendering).unwrap();

        let device_interop: ICompositionGraphicsDeviceInterop = device.cast().unwrap();
        let returned = device_interop.GetRenderingDevice().unwrap();
        assert_eq!(returned.as_raw(), rendering.as_raw());
    }

    #[test]
    fn a_surface_from_the_device_has_the_requested_size() {
        let device = device(&compositor());
        let surface = device
            .CreateDrawingSurface(
                Size {
                    width: 4.0,
                    height: 3.0,
                },
                DirectXPixelFormat::R8G8B8A8UIntNormalizedSrgb,
                DirectXAlphaMode::Premultiplied,
            )
            .unwrap();

        let (width, height, pixels) = crate::surface::surface_pixels(&surface).unwrap();
        assert_eq!((width, height), (4, 3));
        assert_eq!(pixels.len(), 4 * 3 * 4);
    }

    #[test]
    fn begin_draw_refuses_to_make_a_d2d_context() {
        let surface = crate::surface::create(2, 2).unwrap();
        let interop: ICompositionDrawingSurfaceInterop = surface.cast().unwrap();
        let error = interop.BeginDraw(&ID2D1_DEVICE_CONTEXT).unwrap_err();
        assert_eq!(error.code(), E_NOINTERFACE);
    }

    #[test]
    fn resize_through_the_interop_reallocates_the_bitmap() {
        let surface = crate::surface::create(2, 2).unwrap();
        let interop: ICompositionDrawingSurfaceInterop = surface.cast().unwrap();
        interop.Resize(PixelSize { cx: 5, cy: 1 }).unwrap();

        let (width, height, pixels) = crate::surface::surface_pixels(&surface).unwrap();
        assert_eq!((width, height), (5, 1));
        assert_eq!(pixels.len(), 5 * 4);
    }
}
