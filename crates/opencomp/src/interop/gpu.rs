//! The GPU half of a drawing surface.
//!
//! A graphics device receives the client's `ID3D11Device` as an `IUnknown`. This module turns
//! it, once per graphics device, into the chain Direct2D needs (`IDXGIDevice` ->
//! `ID2D1Device` -> `ID2D1DeviceContext`), owns the render-target texture a surface draws
//! into, and copies a finished texture back into the CPU buffer the composition path samples.
//!
//! The client's device arrives as a `windows-core` 0.100 `IUnknown`, while the Direct3D and
//! Direct2D declarations here come from the `windows` 0.62 crate. Both crates describe the
//! same COM ABI, so the reference crosses over through the vtable rather than through a cast
//! of the wrapper types.
//!
//! One context serves every surface of a device, and a Direct2D context has one target, so a
//! surface holds it from `BeginDraw` to `EndDraw` and another surface waits its turn. That is
//! the same serialisation the shipped compositor imposes on its clients.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_TARGET, D2D1_BITMAP_PROPERTIES1,
    D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1CreateFactory,
    ID2D1Bitmap1, ID2D1DeviceContext, ID2D1Factory1,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_USAGE_STAGING, ID3D11Device,
    ID3D11DeviceContext, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{IDXGIDevice, IDXGISurface};
use windows::core::Interface;
use windows_core::IUnknown;

/// The render target a surface draws into: a BGRA8 texture and the bitmap over it.
pub(crate) struct Texture {
    texture: ID3D11Texture2D,
    bitmap: ID2D1Bitmap1,
    width: u32,
    height: u32,
}

/// A drawing target D2D accepts: a target, premultiplied, never drawn from as an image.
fn target_properties() -> D2D1_BITMAP_PROPERTIES1 {
    D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        colorContext: Default::default(),
    }
}

/// The Direct2D context the client draws with, and the D3D objects behind it.
pub(crate) struct GpuDevice {
    d2d: ID2D1DeviceContext,
    d3d: ID3D11Device,
    immediate: ID3D11DeviceContext,
    /// One client draws at a time: the context has a single target.
    busy: AtomicBool,
}

impl GpuDevice {
    /// Turn `device`, the client's rendering device, into the Direct2D objects surfaces use.
    ///
    /// `None` when the object is not a D3D11 device, or when Direct2D refuses it; the
    /// graphics device then keeps working through the CPU path alone.
    pub(crate) fn create(device: &IUnknown) -> Option<Self> {
        // One counted reference crosses the crate boundary: `clone` adds it, `into_raw`
        // hands it over, and the 0.62 wrapper becomes the only owner that releases it.
        let raw = windows_core::Interface::into_raw(device.clone());
        // SAFETY: both crates describe the same COM base vtable and `raw` owns one reference.
        let device: windows::core::IUnknown = unsafe { Interface::from_raw(raw) };

        let dxgi: IDXGIDevice = device.cast().ok()?;
        let d3d: ID3D11Device = device.cast().ok()?;
        let immediate = unsafe { d3d.GetImmediateContext() }.ok()?;
        let factory: ID2D1Factory1 =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }.ok()?;
        let d2d_device = unsafe { factory.CreateDevice(&dxgi) }.ok()?;
        // The context keeps its device alive; neither the factory nor the device is needed
        // afterwards, so only the context is stored.
        let d2d =
            unsafe { d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE) }.ok()?;
        Some(Self {
            d2d,
            d3d,
            immediate,
            busy: AtomicBool::new(false),
        })
    }

    /// A render-target texture of `width` by `height`, or `None` when the device refuses it.
    pub(crate) fn texture(&self, width: u32, height: u32) -> Option<Texture> {
        if width == 0 || height == 0 {
            return None;
        }
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut created = None;
        unsafe { self.d3d.CreateTexture2D(&desc, None, Some(&mut created)) }.ok()?;
        let texture: ID3D11Texture2D = created?;
        let surface: IDXGISurface = texture.cast().ok()?;
        let bitmap = unsafe {
            self.d2d
                .CreateBitmapFromDxgiSurface(&surface, Some(&target_properties()))
        }
        .ok()?;
        Some(Texture {
            texture,
            bitmap,
            width,
            height,
        })
    }

    /// Make `texture` the drawing target and hand out the context, with one reference added.
    ///
    /// `None` when another surface is between its own `BeginDraw` and `EndDraw`.
    pub(crate) fn begin(&self, texture: &Texture) -> Option<*mut c_void> {
        if self.busy.swap(true, Ordering::Acquire) {
            return None;
        }
        unsafe {
            self.d2d.SetTarget(&texture.bitmap);
            self.d2d.SetDpi(96.0, 96.0);
        }
        // The caller becomes the owner of the returned reference.
        Some(Interface::into_raw(self.d2d.clone()))
    }

    /// Close the draw, drop the target, and read the texture back as straight RGBA8.
    ///
    /// A client that ended its own Direct2D draw leaves the context in the wrong state;
    /// that is not an error here, the draw is over either way. `texture` is `None` when the
    /// surface lost its target before the draw closed, and then there is nothing to read.
    pub(crate) fn end(&self, texture: Option<&Texture>) -> Option<Vec<u8>> {
        unsafe {
            let _ = self.d2d.EndDraw(None, None);
            self.d2d.SetTarget(None);
        }
        let pixels = texture.and_then(|texture| self.read(texture));
        self.busy.store(false, Ordering::Release);
        pixels
    }

    /// Copy `texture` into a staging texture and turn its BGRA8 into straight RGBA8.
    fn read(&self, texture: &Texture) -> Option<Vec<u8>> {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { texture.texture.GetDesc(&mut desc) };
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MiscFlags = 0;
        let mut created = None;
        unsafe { self.d3d.CreateTexture2D(&desc, None, Some(&mut created)) }.ok()?;
        let staging: ID3D11Texture2D = created?;
        unsafe { self.immediate.CopyResource(&staging, &texture.texture) };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.immediate
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
        }
        .ok()?;
        let (width, height) = (texture.width as usize, texture.height as usize);
        let mut rgba = vec![0u8; width * height * 4];
        if !mapped.pData.is_null() {
            // SAFETY: the mapped region covers the whole staging texture; every read below
            // stays inside one row of it.
            unsafe {
                for y in 0..height {
                    let row = (mapped.pData as *const u8).add(y * mapped.RowPitch as usize);
                    for x in 0..width {
                        let source = core::slice::from_raw_parts(row.add(x * 4), 4);
                        let target = &mut rgba[(y * width + x) * 4..][..4];
                        target.copy_from_slice(&straight(source));
                    }
                }
            }
        }
        unsafe { self.immediate.Unmap(&staging, 0) };
        Some(rgba)
    }
}

/// Premultiplied BGRA8, as Direct2D leaves it, to the straight RGBA8 the ABI speaks.
fn straight(bgra: &[u8]) -> [u8; 4] {
    let alpha = bgra[3] as u32;
    // A transparent pixel carries no colour; the rest is divided back out of its alpha.
    let unmultiply = |channel: u8| {
        (channel as u32 * 255 + alpha / 2)
            .checked_div(alpha)
            .unwrap_or(0)
            .min(255) as u8
    };
    [
        unmultiply(bgra[2]),
        unmultiply(bgra[1]),
        unmultiply(bgra[0]),
        alpha as u8,
    ]
}
