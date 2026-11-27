//! The Windows compositor: D3D11 for the swapchain, D2D1 for the quads.
//!
//! The shipped stack does the same thing with the same libraries — `dcompi`,
//! `dwmcorei` and `Microsoft.ui.xaml` all import `d3d11`, `d2d1` and `dxgi` — and
//! doing it here rather than with a portable abstraction means a surface is one
//! texture, a quad is one `FillRectangle`, and there is no device to hand over.
//!
//! D2D is also what makes rounded corners, clipping and bitmaps one call each,
//! which is why the compositor has no shader of its own.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use opencomp_core::{Draw, TextureId, srgb_from_linear};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D1_ALPHA_MODE_IGNORE, D2D1_ALPHA_MODE_STRAIGHT, D2D1_COLOR_F, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_BITMAP_OPTIONS_CANNOT_DRAW, D2D1_BITMAP_OPTIONS_NONE, D2D1_BITMAP_OPTIONS_TARGET,
    D2D1_BITMAP_PROPERTIES1, D2D1_DEVICE_CONTEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_INTERPOLATION_MODE_LINEAR, D2D1_ROUNDED_RECT, D2D1CreateFactory, ID2D1Bitmap1,
    ID2D1DeviceContext, ID2D1Factory1, ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_10_0, D3D_FEATURE_LEVEL_10_1,
    D3D_FEATURE_LEVEL_11_0,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
    ID3D11DeviceContext,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_PRESENT, DXGI_SCALING_STRETCH,
    DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_EFFECT_FLIP_DISCARD, DXGI_USAGE_RENDER_TARGET_OUTPUT,
    IDXGIDevice, IDXGIFactory2, IDXGISurface, IDXGISwapChain1,
};
use windows::core::Interface;

/// Frames handed to a window since the process started.
///
/// The ABI has no way for a client to ask "did anything reach the screen?", and a
/// bridge that could not create a device still answers S_OK, so a test needs one
/// number it can assert. This is that number.
static FRAMES: AtomicU64 = AtomicU64::new(0);

pub fn frames_presented() -> u64 {
    FRAMES.load(Ordering::Relaxed)
}

/// The channel order D2D wants: ours is linear light, D2D speaks sRGB.
fn color(rgba: [f32; 4]) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: srgb_from_linear(rgba[0]),
        g: srgb_from_linear(rgba[1]),
        b: srgb_from_linear(rgba[2]),
        a: rgba[3],
    }
}

/// The back buffer's bitmap: a target, never drawn from.
fn target_properties() -> D2D1_BITMAP_PROPERTIES1 {
    D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_B8G8R8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_IGNORE,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
        colorContext: Default::default(),
    }
}

/// A surface's bitmap: straight alpha, because the ABI hands over sRGB bytes.
fn surface_properties() -> D2D1_BITMAP_PROPERTIES1 {
    D2D1_BITMAP_PROPERTIES1 {
        pixelFormat: D2D1_PIXEL_FORMAT {
            format: DXGI_FORMAT_R8G8B8A8_UNORM,
            alphaMode: D2D1_ALPHA_MODE_STRAIGHT,
        },
        dpiX: 96.0,
        dpiY: 96.0,
        bitmapOptions: D2D1_BITMAP_OPTIONS_NONE,
        colorContext: Default::default(),
    }
}

/// A window that draws a draw list: swapchain, D2D context and target bitmap.
pub struct Presenter {
    swapchain: IDXGISwapChain1,
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    d2d: ID2D1DeviceContext,
    brush: ID2D1SolidColorBrush,
    target: Option<ID2D1Bitmap1>,
    bitmaps: HashMap<TextureId, ID2D1Bitmap1>,
    size: (u32, u32),
    scale: f32,
}

impl Presenter {
    /// Build the device, the swapchain and the D2D context for `hwnd`.
    pub fn new(hwnd: u64, width: u32, height: u32) -> Option<Self> {
        let (width, height) = (width.max(1), height.max(1));
        let mut device = None;
        let mut context = None;
        let levels = [
            D3D_FEATURE_LEVEL_11_0,
            D3D_FEATURE_LEVEL_10_1,
            D3D_FEATURE_LEVEL_10_0,
        ];
        // Safety: the out parameters are ours and only read after the call succeeds.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                windows::Win32::Foundation::HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
            .ok()?;
        }
        let device: ID3D11Device = device?;
        let context: ID3D11DeviceContext = context?;

        let factory: IDXGIFactory2 =
            unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }.ok()?;
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            Stereo: false.into(),
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            AlphaMode: Default::default(),
            Flags: 0,
        };
        // Safety: `hwnd` is the window the caller handed over and keeps alive.
        let swapchain = unsafe {
            factory.CreateSwapChainForHwnd(&device, HWND(hwnd as *mut _), &desc, None, None)
        }
        .ok()?;

        let d2d_factory: ID2D1Factory1 =
            unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }.ok()?;
        let dxgi_device: IDXGIDevice = device.cast().ok()?;
        let d2d_device = unsafe { d2d_factory.CreateDevice(&dxgi_device) }.ok()?;
        let d2d =
            unsafe { d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE) }.ok()?;
        let brush =
            unsafe { d2d.CreateSolidColorBrush(&color([1.0, 1.0, 1.0, 1.0]), None) }.ok()?;
        let mut presenter = Self {
            swapchain,
            device,
            context,
            d2d,
            brush,
            target: None,
            bitmaps: HashMap::new(),
            size: (width, height),
            scale: 1.0,
        };
        presenter.bind_target()?;
        Some(presenter)
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// The display scale: composition units are device independent, the swapchain is not.
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.max(0.0);
    }

    /// Recreate the target after the window changed size. The buffers are resized by
    /// DXGI itself; `ResizeBuffers` with the same count keeps the swapchain alive.
    pub fn resize(&mut self, width: u32, height: u32) -> bool {
        let (width, height) = (width.max(1), height.max(1));
        if (width, height) == self.size {
            return true;
        }
        // D2D holds its own reference to the target bitmap, and DXGI will not resize a
        // swapchain whose back buffer is still referenced. Release the target first.
        unsafe {
            let _ = self.d2d.EndDraw(None, None);
            self.d2d.SetTarget(None);
        }
        self.target = None;
        // Safety: no target references the back buffer any more.
        let resized = unsafe {
            self.swapchain.ResizeBuffers(
                2,
                width,
                height,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                Default::default(),
            )
        };
        self.size = (width, height);
        resized.is_ok() && self.bind_target().is_some()
    }

    fn bind_target(&mut self) -> Option<()> {
        // Safety: buffer zero of a flip-model swapchain is a DXGI surface.
        let surface: IDXGISurface = unsafe { self.swapchain.GetBuffer(0) }.ok()?;
        let properties = target_properties();
        // Safety: the surface belongs to this swapchain and lives as long as the target.
        let target = unsafe {
            self.d2d
                .CreateBitmapFromDxgiSurface(&surface, Some(&properties))
        }
        .ok()?;
        unsafe {
            self.d2d.SetTarget(&target);
            self.d2d.SetDpi(96.0, 96.0);
        }
        self.target = Some(target);
        Some(())
    }

    /// Replace a surface's pixels. sRGB RGBA8, row major, as the ABI hands them over.
    pub fn upload(&mut self, id: TextureId, width: u32, height: u32, pixels: &[u8]) -> bool {
        if id == 0 || width == 0 || height == 0 || pixels.len() < (width * height * 4) as usize {
            return false;
        }
        let properties = surface_properties();
        // Safety: `pixels` is at least `width * height * 4` bytes, checked above, and D2D
        // copies it before returning.
        let bitmap = unsafe {
            self.d2d.CreateBitmap(
                windows::Win32::Graphics::Direct2D::Common::D2D_SIZE_U { width, height },
                Some(pixels.as_ptr().cast()),
                width * 4,
                &properties,
            )
        };
        match bitmap {
            Ok(bitmap) => {
                self.bitmaps.insert(id, bitmap);
                true
            }
            Err(_) => false,
        }
    }

    /// The colour of one pixel of the frame just drawn, in sRGB RGBA.
    ///
    /// A flip-model back buffer is not readable through GDI, so a test that wants to
    /// check a pixel has to copy the buffer to a staging texture. Only tests do that.
    #[cfg(test)]
    pub fn read_pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        use windows::Win32::Graphics::Direct3D11::{
            D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_USAGE_STAGING,
            ID3D11Texture2D,
        };

        // Safety: buffer zero belongs to this swapchain and is still ours until Present.
        let back: ID3D11Texture2D = unsafe { self.swapchain.GetBuffer(0) }.ok()?;
        let mut desc = windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC::default();
        unsafe { back.GetDesc(&mut desc) };
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        desc.MiscFlags = 0;
        let mut staging = None;
        unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut staging)) }.ok()?;
        let staging: ID3D11Texture2D = staging?;
        unsafe { self.context.CopyResource(&staging, &back) };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
        }
        .ok()?;
        if mapped.pData.is_null() {
            return None;
        }
        let offset = y as usize * mapped.RowPitch as usize + x as usize * 4;
        let mut pixel = [0u8; 4];
        // Safety: the mapped region covers the whole texture; x and y are inside it.
        unsafe {
            let bytes = mapped.pData.cast::<u8>().add(offset);
            pixel.copy_from_slice(core::slice::from_raw_parts(bytes, 4));
            self.context.Unmap(&staging, 0);
        }
        // The surface is BGRA; callers want RGBA.
        Some([pixel[2], pixel[1], pixel[0], pixel[3]])
    }

    /// Draw the list and present it.
    pub fn present(&mut self, draws: &[Draw]) -> bool {
        self.draw(draws) && self.present_frame()
    }

    /// Draw the list into the back buffer without handing it to the window.
    pub fn draw(&mut self, draws: &[Draw]) -> bool {
        unsafe {
            self.d2d.BeginDraw();
            self.d2d.Clear(Some(&color([0.0, 0.0, 0.0, 1.0])));
            let scale = if self.scale > 0.0 { self.scale } else { 1.0 };
            for draw in draws {
                let bounds = D2D_RECT_F {
                    left: draw.scissor.x * scale,
                    top: draw.scissor.y * scale,
                    right: (draw.scissor.x + draw.scissor.width) * scale,
                    bottom: (draw.scissor.y + draw.scissor.height) * scale,
                };
                if bounds.right <= bounds.left || bounds.bottom <= bounds.top {
                    continue;
                }
                self.d2d.PushAxisAlignedClip(&bounds, Default::default());
                match draw.image.and_then(|id| self.bitmaps.get(&id)) {
                    Some(bitmap) => {
                        let (left, top) = (draw.corners[0][0] * scale, draw.corners[0][1] * scale);
                        let (right, bottom) =
                            (draw.corners[2][0] * scale, draw.corners[2][1] * scale);
                        let destination = D2D_RECT_F {
                            left,
                            top,
                            right,
                            bottom,
                        };
                        let pixel = bitmap.GetPixelSize();
                        let source = D2D_RECT_F {
                            left: draw.uv[0] * pixel.width as f32,
                            top: draw.uv[1] * pixel.height as f32,
                            right: draw.uv[2] * pixel.width as f32,
                            bottom: draw.uv[3] * pixel.height as f32,
                        };
                        self.d2d.DrawBitmap(
                            bitmap,
                            Some(&destination),
                            draw.color[3],
                            D2D1_INTERPOLATION_MODE_LINEAR,
                            Some(&source),
                            None,
                        );
                    }
                    None => {
                        self.brush.SetColor(&color(draw.color));
                        let (left, top) = (draw.corners[0][0] * scale, draw.corners[0][1] * scale);
                        let (right, bottom) =
                            (draw.corners[2][0] * scale, draw.corners[2][1] * scale);
                        // D2D has one radius for all four corners; the largest wins.
                        let radius = draw.radius.iter().copied().fold(0.0, f32::max) * scale;
                        if radius > 0.0 {
                            let rounded = D2D1_ROUNDED_RECT {
                                rect: D2D_RECT_F {
                                    left,
                                    top,
                                    right,
                                    bottom,
                                },
                                radiusX: radius,
                                radiusY: radius,
                            };
                            self.d2d.FillRoundedRectangle(&rounded, &self.brush);
                        } else {
                            let rect = D2D_RECT_F {
                                left,
                                top,
                                right,
                                bottom,
                            };
                            self.d2d.FillRectangle(&rect, &self.brush);
                        }
                    }
                }
                self.d2d.PopAxisAlignedClip();
            }
            self.d2d.EndDraw(None, None).is_ok()
        }
    }

    /// Hand the drawn back buffer to the window.
    pub fn present_frame(&mut self) -> bool {
        // Interval zero: a compositor paces itself, and under Wine's software backend
        // `Present(1, ..)` waits for a vertical blank that Xvfb never sends. Not
        // DO_NOT_WAIT either -- that fails while the previous frame is still in flight,
        // which a caller has no way to tell apart from a real error.
        // Safety: the frame was drawn; DXGI takes it from here.
        let presented = unsafe { self.swapchain.Present(0, DXGI_PRESENT(0)) }.is_ok();
        if presented {
            FRAMES.fetch_add(1, Ordering::Relaxed);
        }
        presented
    }
}

#[cfg(all(test, windows))]
#[path = "d2d_tests.rs"]
mod tests;
