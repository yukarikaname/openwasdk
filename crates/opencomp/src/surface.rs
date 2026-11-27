//! `CompositionDrawingSurface`: the CPU bitmap a surface brush draws.
//!
//! A surface owns an RGBA8 buffer, a size, and the pixel format and alpha mode it was created
//! with. A surface a graphics device creates also owns a BGRA8 render-target texture over the
//! client's D3D11 device, and the `ICompositionDrawingSurfaceInterop` half hands its Direct2D
//! device context to a client; after `EndDraw` the texture is copied back into the buffer, so
//! the composition path samples the CPU copy whether or not the client drew through Direct2D.
//! The path for a host that owns the pixels is [`create`], [`surface_update`],
//! [`surface_pixels`] and [`surface_image`].
//!
//! A surface id is also a texture id. Ids come from one global counter starting at one, so zero
//! stays the renderer's built-in white texture. The DLL does not hold a renderer, so the host
//! takes the buffer reported by [`surface_image`] and uploads it with `Renderer::upload_rgba`
//! under that id before presenting.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use crate::bindings::Microsoft::Graphics::DirectX::{DirectXAlphaMode, DirectXPixelFormat};
use crate::bindings::Microsoft::UI::Composition::{
    ICompositionDrawingSurface, ICompositionDrawingSurface_Impl, ICompositionDrawingSurface2,
    ICompositionDrawingSurface2_Impl, ICompositionObject, ICompositionObject2, ICompositionObject4,
    ICompositionObject5, ICompositionSurface, ICompositionSurface_Impl,
};
use crate::bindings::Windows::Foundation::Size;
use crate::bindings::Windows::Graphics::SizeInt32;
use crate::interop::{GpuDevice, ICompositionDrawingSurfaceInterop, Texture};
use crate::object::{composition_object, not_implemented};
use windows_core::{Error, HRESULT, Interface, Result};

mod draw;

/// The ABI class a host passes back to [`surface_update`] and [`surface_pixels`].
pub use crate::bindings::Microsoft::UI::Composition::CompositionDrawingSurface;

/// What Direct2D allows, and a bound an allocation from client input cannot cross.
const MAX_DIMENSION: u32 = 16384;

const E_OUTOFMEMORY: HRESULT = HRESULT(0x8007_000eu32 as i32);
const E_INVALIDARG: HRESULT = HRESULT(0x8007_0057u32 as i32);

/// The next surface id. Zero is the renderer's white texture, so the counter starts at one.
static NEXT_ID: AtomicU32 = AtomicU32::new(1);

/// One surface in the id table; the entry is weak, so it dies with the surface.
type Entry = (u32, Weak<DrawingSurface>);

/// Surfaces by id, for a host that has a node but not the COM object. Entries are weak, like the
/// identity table: a surface that dies leaves nothing behind.
static REGISTRY: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();

fn registry() -> &'static Mutex<Vec<Entry>> {
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

fn next_id() -> u32 {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    // The counter wraps only after four billion surfaces; zero is never handed out.
    if id == 0 { 1 } else { id }
}

fn publish_id(surface: &Arc<DrawingSurface>) {
    let Ok(mut table) = registry().lock() else {
        return;
    };
    if table.len() >= 64 {
        table.retain(|(_, surface)| surface.strong_count() > 0);
    }
    table.push((surface.id, Arc::downgrade(surface)));
}

fn find(id: u32) -> Option<Arc<DrawingSurface>> {
    let table = registry().lock().ok()?;
    table
        .iter()
        .find(|(existing, _)| *existing == id)
        .and_then(|(_, surface)| surface.upgrade())
}

/// The pixels of one surface, always with the size they fill.
struct Buffer {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Buffer {
    /// A zeroed buffer, or `None` when the size cannot be allocated.
    fn new(width: u32, height: u32) -> Option<Self> {
        let width = width.min(MAX_DIMENSION);
        let height = height.min(MAX_DIMENSION);
        let len = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(len).ok()?;
        rgba.resize(len, 0);
        Some(Self {
            width,
            height,
            rgba,
        })
    }
}

/// The state behind a `CompositionDrawingSurface`, held by the COM object and the id table.
struct DrawingSurface {
    id: u32,
    pixels: Mutex<Buffer>,
    // Fixed at creation: the buffer is always RGBA8, whatever the requested byte order.
    format: DirectXPixelFormat,
    alpha: DirectXAlphaMode,
    /// The Direct2D objects of the device that made this surface; `None` on the CPU path.
    gpu: Option<Arc<GpuDevice>>,
    /// The render-target texture over `gpu`'s device, rebuilt by [`resize`].
    texture: Mutex<Option<Texture>>,
    /// Whether a `BeginDraw` is open; one session per surface at a time.
    drawing: AtomicBool,
}

impl DrawingSurface {
    fn size(&self) -> (u32, u32) {
        match self.pixels.lock() {
            Ok(buffer) => (buffer.width, buffer.height),
            Err(_) => (0, 0),
        }
    }

    fn snapshot(&self) -> (u32, u32, Vec<u8>) {
        match self.pixels.lock() {
            Ok(buffer) => (buffer.width, buffer.height, buffer.rgba.clone()),
            Err(_) => (0, 0, Vec::new()),
        }
    }

    /// A texture of `width` by `height` over the surface's device, if it has one.
    fn make_texture(&self, width: u32, height: u32) -> Option<Texture> {
        self.gpu.as_ref()?.texture(width, height)
    }

    fn resize(&self, width: u32, height: u32) -> bool {
        let Some(buffer) = Buffer::new(width, height) else {
            return false;
        };
        // A texture is the same size as the buffer it mirrors; the old one is dropped here.
        let texture = self.make_texture(buffer.width, buffer.height);
        let Ok(mut current) = self.texture.lock() else {
            return false;
        };
        *current = texture;
        drop(current);
        match self.pixels.lock() {
            Ok(mut current) => {
                *current = buffer;
                true
            }
            Err(_) => false,
        }
    }

    fn update(&self, rgba: &[u8]) -> bool {
        let Ok(mut buffer) = self.pixels.lock() else {
            return false;
        };
        if rgba.len() != buffer.rgba.len() {
            return false;
        }
        buffer.rgba.copy_from_slice(rgba);
        true
    }
}

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ICompositionDrawingSurface,
    ICompositionDrawingSurface2,
    ICompositionDrawingSurfaceInterop,
    ICompositionSurface
)]
pub struct DrawingSurfaceObject {
    surface: Arc<DrawingSurface>,
}

composition_object!(DrawingSurfaceObject_Impl);

impl ICompositionSurface_Impl for DrawingSurfaceObject_Impl {}

impl ICompositionDrawingSurface_Impl for DrawingSurfaceObject_Impl {
    fn AlphaMode(&self) -> Result<DirectXAlphaMode> {
        Ok(self.surface.alpha)
    }

    fn PixelFormat(&self) -> Result<DirectXPixelFormat> {
        Ok(self.surface.format)
    }

    fn Size(&self) -> Result<Size> {
        let (width, height) = self.surface.size();
        Ok(Size {
            width: width as f32,
            height: height as f32,
        })
    }
}

impl ICompositionDrawingSurface2_Impl for DrawingSurfaceObject_Impl {
    fn SizeInt32(&self) -> Result<SizeInt32> {
        let (width, height) = self.surface.size();
        Ok(SizeInt32 {
            width: width as i32,
            height: height as i32,
        })
    }

    fn Resize(&self, sizePixels: &SizeInt32) -> Result<()> {
        let width = sizePixels.width.max(0) as u32;
        let height = sizePixels.height.max(0) as u32;
        if self.surface.resize(width, height) {
            Ok(())
        } else {
            Err(Error::from_hresult(E_OUTOFMEMORY))
        }
    }

    // Scrolling would have to move pixels the host may be filling; it is not part of this path.
    fn Scroll(&self, _offset: &crate::bindings::Windows::Graphics::PointInt32) -> Result<()> {
        not_implemented()
    }

    fn ScrollRect(
        &self,
        _offset: &crate::bindings::Windows::Graphics::PointInt32,
        _scrollRect: &crate::bindings::Windows::Graphics::RectInt32,
    ) -> Result<()> {
        not_implemented()
    }

    fn ScrollWithClip(
        &self,
        _offset: &crate::bindings::Windows::Graphics::PointInt32,
        _clipRect: &crate::bindings::Windows::Graphics::RectInt32,
    ) -> Result<()> {
        not_implemented()
    }

    fn ScrollRectWithClip(
        &self,
        _offset: &crate::bindings::Windows::Graphics::PointInt32,
        _clipRect: &crate::bindings::Windows::Graphics::RectInt32,
        _scrollRect: &crate::bindings::Windows::Graphics::RectInt32,
    ) -> Result<()> {
        not_implemented()
    }
}

/// A surface of `width` by `height` pixels, zeroed, for a host to fill.
///
/// The host path is the one this crate offers directly; a WinRT client reaches
/// [`create_sized`] through the graphics device.
pub fn create(width: u32, height: u32) -> Result<CompositionDrawingSurface> {
    create_with(
        width,
        height,
        DirectXPixelFormat::R8G8B8A8UIntNormalizedSrgb,
        DirectXAlphaMode::Straight,
        None,
    )
}

/// A surface the size a graphics device was asked for, in the requested format.
///
/// The buffer is RGBA8 either way, so the two RGBA8 formats are the ones a client may ask
/// for; anything else fails with `E_INVALIDARG`. `gpu`
/// is the device's Direct2D half, or `None` for the CPU-only path.
pub(crate) fn create_sized(
    size: Size,
    format: DirectXPixelFormat,
    alpha: DirectXAlphaMode,
    gpu: Option<Arc<GpuDevice>>,
) -> Result<CompositionDrawingSurface> {
    // Both channel orders are accepted: XAML asks for BGRA, our own tests for RGBA.
    // The GPU texture is BGRA either way and the readback converts to the RGBA the
    // composition path samples, so the requested format is what the getter echoes.
    match format {
        DirectXPixelFormat::R8G8B8A8UIntNormalized
        | DirectXPixelFormat::R8G8B8A8UIntNormalizedSrgb
        | DirectXPixelFormat::B8G8R8A8UIntNormalized
        | DirectXPixelFormat::B8G8R8A8UIntNormalizedSrgb => {}
        _ => return Err(Error::from_hresult(E_INVALIDARG)),
    }
    let width = size.width.max(0.0) as u32;
    let height = size.height.max(0.0) as u32;
    create_with(width, height, format, alpha, gpu)
}

fn create_with(
    width: u32,
    height: u32,
    format: DirectXPixelFormat,
    alpha: DirectXAlphaMode,
    gpu: Option<Arc<GpuDevice>>,
) -> Result<CompositionDrawingSurface> {
    let buffer = Buffer::new(width, height).ok_or_else(|| Error::from_hresult(E_OUTOFMEMORY))?;
    // A failed allocation leaves the surface on the CPU path instead of failing creation.
    let texture = gpu
        .as_ref()
        .and_then(|gpu| gpu.texture(buffer.width, buffer.height));
    let surface = Arc::new(DrawingSurface {
        id: next_id(),
        pixels: Mutex::new(buffer),
        format,
        alpha,
        gpu,
        texture: Mutex::new(texture),
        drawing: AtomicBool::new(false),
    });
    let object: ICompositionDrawingSurface = DrawingSurfaceObject {
        surface: surface.clone(),
    }
    .into();
    crate::identity::publish(&object, &surface);
    publish_id(&surface);
    object.cast()
}

/// Copy `rgba` into the surface, sRGB RGBA8 and row-major.
///
/// The slice has to be exactly as long as the surface; a shorter or longer one is ignored.
pub fn surface_update(surface: &CompositionDrawingSurface, rgba: &[u8]) -> bool {
    match crate::identity::lookup::<_, DrawingSurface>(surface) {
        Some(surface) => surface.update(rgba),
        None => false,
    }
}

/// The surface's size and a copy of its pixels, sRGB RGBA8 and row-major.
pub fn surface_pixels(surface: &CompositionDrawingSurface) -> Option<(u32, u32, Vec<u8>)> {
    let surface = crate::identity::lookup::<_, DrawingSurface>(surface)?;
    Some(surface.snapshot())
}

/// The bitmap `node` draws, as its texture id, width, height and pixels.
///
/// The id is the one the surface brush copied onto the node; uploading the pixels under it is
/// the host's job, because the DLL does not own the renderer.
pub fn surface_image(node: &crate::node::NodeRef) -> Option<(u32, u32, u32, Vec<u8>)> {
    let id = node.lock().ok()?.surface?;
    let surface = find(id)?;
    let (width, height, pixels) = surface.snapshot();
    Some((id, width, height, pixels))
}

/// The id and size of a surface a client handed in as a brush's `Surface`.
pub(crate) fn describe(surface: &ICompositionSurface) -> Option<(u32, u32, u32)> {
    let surface = crate::identity::lookup::<_, DrawingSurface>(surface)?;
    let (width, height) = surface.size();
    Some((surface.id, width, height))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn a_new_surface_is_zeroed_and_sized() {
        let surface = create(2, 3).unwrap();
        let (width, height, pixels) = surface_pixels(&surface).unwrap();
        assert_eq!((width, height), (2, 3));
        assert_eq!(pixels, vec![0; 2 * 3 * 4]);
    }

    #[test]
    fn resize_reallocates_the_buffer() {
        let surface = create(2, 2).unwrap();
        let draw: ICompositionDrawingSurface2 = surface.cast().unwrap();
        draw.Resize(SizeInt32 {
            width: 4,
            height: 1,
        })
        .unwrap();

        let (width, height, pixels) = surface_pixels(&surface).unwrap();
        assert_eq!((width, height), (4, 1));
        assert_eq!(pixels.len(), 16);
        assert_eq!(
            draw.SizeInt32().unwrap(),
            SizeInt32 {
                width: 4,
                height: 1
            }
        );
    }

    #[test]
    fn a_negative_resize_clamps_to_nothing() {
        let surface = create(2, 2).unwrap();
        let draw: ICompositionDrawingSurface2 = surface.cast().unwrap();
        draw.Resize(SizeInt32 {
            width: -4,
            height: 2,
        })
        .unwrap();

        let (width, height, pixels) = surface_pixels(&surface).unwrap();
        assert_eq!((width, height), (0, 2));
        assert!(pixels.is_empty());
    }

    #[test]
    fn the_interface_reports_the_format() {
        let surface = create(1, 1).unwrap();
        let format: ICompositionDrawingSurface = surface.cast().unwrap();
        assert_eq!(format.AlphaMode().unwrap(), DirectXAlphaMode::Straight);
        assert_eq!(
            format.PixelFormat().unwrap(),
            DirectXPixelFormat::R8G8B8A8UIntNormalizedSrgb
        );
        assert_eq!(
            format.Size().unwrap(),
            Size {
                width: 1.0,
                height: 1.0
            }
        );
    }

    #[test]
    fn an_update_of_the_wrong_length_is_ignored() {
        let surface = create(1, 1).unwrap();
        assert!(!surface_update(&surface, &[1, 2, 3]));
        assert!(surface_update(&surface, &[1, 2, 3, 4]));
        assert_eq!(surface_pixels(&surface).unwrap().2, vec![1, 2, 3, 4]);
    }
}
