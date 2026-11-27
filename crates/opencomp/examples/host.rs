//! The composition-only host: a window, an island and the renderer, in one process.
//!
//! Nothing here is XAML. The example opens a winit window, asks this DLL for the composition
//! classes the way a client does, builds a small tree through their ABI, wraps the root in a
//! `ContentIsland`, attaches it to a `DesktopChildSiteBridge` for the window's HWND, and then
//! draws the island on every redraw with `opencomp_core::Renderer`.
//!
//! `cargo run -p opencomp --example host`
//!
//! `OPENCOMP_FRAMES=N` exits after N presented frames, which is how it runs unattended.

#[cfg(not(windows))]
fn main() {
    println!("opencomp host: this example needs Windows; the composition ABI is Windows only");
}

#[cfg(windows)]
fn main() {
    if let Err(error) = windows::run() {
        eprintln!("opencomp host: {error}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;
    use std::ptr;
    use std::sync::Arc;

    use opencomp::bindings::Microsoft::UI::Composition::{
        CompositionBrush, CompositionClip, Compositor, ICompositionSurface, ICompositor5,
        ICompositor6, IContainerVisual, ISpriteVisual, IVisual, IVisual2, SpriteVisual, Visual,
    };
    use opencomp::bindings::Microsoft::UI::Content::{
        ContentIsland, ContentSizePolicy, DesktopChildSiteBridge, IContentIslandStatics,
        IDesktopChildSiteBridge, IDesktopChildSiteBridgeStatics, IDesktopSiteBridge,
    };
    use opencomp::bindings::Microsoft::UI::WindowId as ContentWindowId;
    use opencomp::bindings::Windows::Graphics::RectInt32;
    use opencomp::bindings::Windows::UI::Color;
    use opencomp::node::NodeRef;
    use opencomp::{content, node, surface};
    use opencomp_core::{Renderer, srgb};
    use windows_core::{Error, HRESULT, HSTRING, IUnknown, Interface};
    use windows_numerics::{Vector2, Vector3};
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::window::{Window, WindowId};

    type Failure = Box<dyn std::error::Error>;

    const COMPOSITOR_CLASS: &str = "Microsoft.UI.Composition.Compositor";
    const ISLAND_CLASS: &str = "Microsoft.UI.Content.ContentIsland";
    const BRIDGE_CLASS: &str = "Microsoft.UI.Content.DesktopChildSiteBridge";

    const CHECKER_SIZE: u32 = 64;
    const CHECKER_CELL: u32 = 8;

    // `IActivationFactory` is not part of the generated metadata: activation lives on the hand
    // written factory in `factory.rs`, which only WinRT itself calls. Declaring the interface
    // here is what a C++/WinRT client gets from `winrt/Windows.Foundation.h`; only the method
    // name follows Rust's spelling, the vtable slot is the ABI's.
    windows_core::interface_decl! {
        unsafe trait IActivationFactory(
            IActivationFactoryVtbl,
            IActivationFactoryImpl
        ) : windows_core::IUnknown
            = 0x0000_0035_0000_0000_c000_0000_0000_0046
        {
            unsafe fn activate_instance(
                &self,
                instance: *mut *mut core::ffi::c_void,
            ) -> Result<()>;
        }
    }

    pub fn run() -> Result<(), Failure> {
        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Wait);
        let limit = std::env::var("OPENCOMP_FRAMES")
            .ok()
            .and_then(|value| value.parse().ok());
        event_loop.run_app(&mut App {
            limit,
            ..App::default()
        })?;
        Ok(())
    }

    /// The factory the DLL exports for `class`, reached through the export rather than the
    /// runtime, because this process hosts the DLL itself.
    fn factory(class: &str) -> Result<IUnknown, Failure> {
        let name = HSTRING::from(class);
        // WinRT passes the HSTRING handle by value, which is the pointer inside the HSTRING.
        // Copy it the way the loading code does; `name` still owns and releases its reference.
        let handle: *mut c_void = unsafe { std::mem::transmute_copy(&name) };
        let mut raw: *mut c_void = ptr::null_mut();
        let result = unsafe { opencomp::DllGetActivationFactory(handle, &mut raw) };
        if result < 0 {
            return Err(Error::from_hresult(HRESULT(result)).into());
        }
        Ok(unsafe { IUnknown::from_raw(raw) })
    }

    /// A default-constructible runtime class: the compositor.
    fn activate<T: Interface>(class: &str) -> Result<T, Failure> {
        let factory: IActivationFactory = factory(class)?.cast()?;
        let mut raw: *mut c_void = ptr::null_mut();
        unsafe { factory.activate_instance(&mut raw) }?;
        Ok(unsafe { IUnknown::from_raw(raw) }.cast()?)
    }

    /// The tree the island wraps: a container that fills the site, a rounded colour sprite, a
    /// plain colour sprite, and a sprite painted by a surface the host fills with a checkerboard.
    fn build(compositor: &Compositor) -> Result<Visual, Failure> {
        let root = compositor.CreateContainerVisual()?;
        let visual: IVisual2 = root.cast()?;
        visual.SetRelativeSizeAdjustment(Vector2 { x: 1.0, y: 1.0 })?;
        let container: IContainerVisual = root.cast()?;
        let children = container.Children()?;

        let rounded = compositor.CreateSpriteVisual()?;
        let visual: IVisual = rounded.cast()?;
        visual.SetOffset(Vector3 {
            x: 40.0,
            y: 40.0,
            z: 0.0,
        })?;
        visual.SetSize(Vector2 { x: 240.0, y: 140.0 })?;
        paint(
            compositor,
            &rounded,
            Color {
                a: 255,
                r: 76,
                g: 132,
                b: 224,
            },
        )?;
        rounded_clip(compositor, &visual, Vector2 { x: 240.0, y: 140.0 }, 28.0)?;
        children.InsertAtTop(&rounded)?;

        let plain = compositor.CreateSpriteVisual()?;
        let visual: IVisual = plain.cast()?;
        visual.SetOffset(Vector3 {
            x: 320.0,
            y: 40.0,
            z: 0.0,
        })?;
        visual.SetSize(Vector2 { x: 180.0, y: 140.0 })?;
        paint(
            compositor,
            &plain,
            Color {
                a: 255,
                r: 244,
                g: 170,
                b: 70,
            },
        )?;
        children.InsertAtTop(&plain)?;

        let bitmap = surface::create(CHECKER_SIZE, CHECKER_SIZE)?;
        if !surface::surface_update(&bitmap, &checkerboard()) {
            return Err("the drawing surface refused the checkerboard".into());
        }
        let surface: ICompositionSurface = bitmap.cast()?;
        let brush = compositor.CreateSurfaceBrushWithSurface(&surface)?;
        let brush: CompositionBrush = brush.cast()?;
        let checker = compositor.CreateSpriteVisual()?;
        let visual: IVisual = checker.cast()?;
        visual.SetOffset(Vector3 {
            x: 40.0,
            y: 220.0,
            z: 0.0,
        })?;
        visual.SetSize(Vector2 { x: 200.0, y: 200.0 })?;
        let sprite: ISpriteVisual = checker.cast()?;
        sprite.SetBrush(&brush)?;
        children.InsertAtTop(&checker)?;

        Ok(root.cast()?)
    }

    /// Give `sprite` a colour brush; the brush copies its colour into the visual when set.
    fn paint(compositor: &Compositor, sprite: &SpriteVisual, color: Color) -> Result<(), Failure> {
        let brush = compositor.CreateColorBrushWithColor(color)?;
        let brush: CompositionBrush = brush.cast()?;
        let visual: ISpriteVisual = sprite.cast()?;
        Ok(visual.SetBrush(&brush)?)
    }

    /// A geometric clip around a rounded rectangle the size of the visual.
    ///
    /// The renderer has no clip path: it rounds the visual's own corners and clips its children
    /// to a rectangle, which is what a rounded rectangle clip of the sprite is.
    fn rounded_clip(
        compositor: &Compositor,
        visual: &IVisual,
        size: Vector2,
        radius: f32,
    ) -> Result<(), Failure> {
        let shape = compositor
            .cast::<ICompositor5>()?
            .CreateRoundedRectangleGeometry()?;
        shape.SetSize(size)?;
        shape.SetCornerRadius(Vector2 {
            x: radius,
            y: radius,
        })?;
        let clip = compositor
            .cast::<ICompositor6>()?
            .CreateGeometricClipWithGeometry(&shape)?;
        let clip: CompositionClip = clip.cast()?;
        Ok(visual.SetClip(&clip)?)
    }

    /// Two sRGB grey levels in eight pixel cells, ready for `surface_update`.
    fn checkerboard() -> Vec<u8> {
        let mut pixels = Vec::with_capacity((CHECKER_SIZE * CHECKER_SIZE * 4) as usize);
        for y in 0..CHECKER_SIZE {
            for x in 0..CHECKER_SIZE {
                let value = if (x / CHECKER_CELL + y / CHECKER_CELL).is_multiple_of(2) {
                    0xd8
                } else {
                    0x40
                };
                pixels.extend_from_slice(&[value, value, value, 255]);
            }
        }
        pixels
    }

    /// The island for `root`, created through `IContentIslandStatics::Create`.
    fn create_island(root: &Visual) -> Result<ContentIsland, Failure> {
        let statics: IContentIslandStatics = factory(ISLAND_CLASS)?.cast()?;
        let mut raw: *mut c_void = ptr::null_mut();
        // The generator emits no safe wrapper for a statics interface, so this is the vtable
        // call a C++/WinRT client does not see but makes all the same.
        unsafe {
            (Interface::vtable(&statics).Create)(
                Interface::as_raw(&statics),
                Interface::as_raw(root),
                &mut raw,
            )
        }
        .ok()?;
        Ok(unsafe { ContentIsland::from_raw(raw) })
    }

    /// The child site bridge for `window`, created through `IDesktopChildSiteBridgeStatics`.
    fn create_bridge(
        compositor: &Compositor,
        window: ContentWindowId,
    ) -> Result<DesktopChildSiteBridge, Failure> {
        let statics: IDesktopChildSiteBridgeStatics = factory(BRIDGE_CLASS)?.cast()?;
        let mut raw: *mut c_void = ptr::null_mut();
        unsafe {
            (Interface::vtable(&statics).Create)(
                Interface::as_raw(&statics),
                Interface::as_raw(compositor),
                window,
                &mut raw,
            )
        }
        .ok()?;
        Ok(unsafe { DesktopChildSiteBridge::from_raw(raw) })
    }

    /// The HWND reinterpreted as the ABI's `WindowId`, the conversion FrameworkUdk performs.
    fn window_id(window: &Window) -> Result<ContentWindowId, Failure> {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

        match window.window_handle()?.as_raw() {
            RawWindowHandle::Win32(handle) => Ok(ContentWindowId {
                value: handle.hwnd.get() as u64,
            }),
            _ => Err("the window is not a Win32 window".into()),
        }
    }

    /// The island and the site that places it in the window.
    struct Host {
        island: ContentIsland,
        site: IDesktopSiteBridge,
    }

    impl Host {
        fn new(window: &Window) -> Result<Self, Failure> {
            let compositor: Compositor = activate(COMPOSITOR_CLASS)?;
            let root = build(&compositor)?;
            let island = create_island(&root)?;
            let bridge = create_bridge(&compositor, window_id(window)?)?;

            let site: IDesktopSiteBridge = bridge.cast()?;
            site.Connect(&island)?;
            let child: IDesktopChildSiteBridge = bridge.cast()?;
            child.SetResizePolicy(ContentSizePolicy::ResizeContentToParentWindow)?;
            site.Show()?;
            Ok(Self { island, site })
        }

        /// Follow the window: the bridge records the rectangle and sizes the island with it.
        fn resize(&self, width: u32, height: u32) {
            let _ = self.site.MoveAndResize(RectInt32 {
                x: 0,
                y: 0,
                width: width as i32,
                height: height as i32,
            });
        }
    }

    /// Upload every surface the tree names; `surface_image` is the only way the host learns the
    /// texture id a surface brush copied onto a node.
    fn upload(renderer: &mut Renderer, node: &NodeRef) {
        if let Some((id, width, height, pixels)) = surface::surface_image(node) {
            renderer.upload_rgba(id, width, height, &pixels);
        }
        // Copy the child list before recursing: the lock must not be held across the walk.
        let children = node.lock().unwrap().children.clone();
        for child in children {
            upload(renderer, &child);
        }
    }

    struct Frame {
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        renderer: Renderer,
    }

    impl Frame {
        fn new(window: Arc<Window>) -> Option<Self> {
            let size = window.inner_size();
            let (width, height) = (size.width.max(1), size.height.max(1));
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(
                    Box::new(window.clone()),
                ));
            let surface = instance.create_surface(window).ok()?;
            let adapter =
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    compatible_surface: Some(&surface),
                    ..Default::default()
                }))
                .ok()?;
            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                    .ok()?;
            let capabilities = surface.get_capabilities(&adapter);
            let format = capabilities
                .formats
                .iter()
                .copied()
                .find(wgpu::TextureFormat::is_srgb)
                .unwrap_or(capabilities.formats[0]);
            let mut config = surface.get_default_config(&adapter, width, height)?;
            config.format = format;
            surface.configure(&device, &config);
            let renderer = Renderer::with_format(
                device.clone(),
                queue.clone(),
                (width, height),
                [srgb(0.09), srgb(0.10), srgb(0.12), 1.0],
                format,
            );
            Some(Self {
                device,
                queue,
                surface,
                config,
                renderer,
            })
        }

        fn resize(&mut self, width: u32, height: u32) {
            if width == 0 || height == 0 {
                return;
            }
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.renderer.set_size((width, height));
        }
    }

    #[derive(Default)]
    struct App {
        window: Option<Arc<Window>>,
        frame: Option<Frame>,
        host: Option<Host>,
        presented: u32,
        limit: Option<u32>,
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            let window = Arc::new(
                event_loop
                    .create_window(Window::default_attributes().with_title("opencomp host"))
                    .expect("window"),
            );
            let Some(frame) = Frame::new(window.clone()) else {
                eprintln!("opencomp host: no wgpu adapter for this window");
                event_loop.exit();
                return;
            };
            let host = match Host::new(&window) {
                Ok(host) => host,
                Err(error) => {
                    eprintln!("opencomp host: {error}");
                    event_loop.exit();
                    return;
                }
            };
            self.window = Some(window);
            self.frame = Some(frame);
            self.host = Some(host);
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _id: WindowId,
            event: WindowEvent,
        ) {
            let Some(frame) = &mut self.frame else { return };
            let Some(host) = &self.host else { return };
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(size) => {
                    frame.resize(size.width, size.height);
                    host.resize(size.width, size.height);
                }
                WindowEvent::RedrawRequested => {
                    let Some(node) = content::root_node(&host.island) else {
                        return;
                    };
                    upload(&mut frame.renderer, &node);
                    let scale = self
                        .window
                        .as_ref()
                        .map_or(1.0, |window| window.scale_factor() as f32);
                    // The swapchain is in device pixels and the tree in composition units; the
                    // renderer applies `scale`, so the root resolves against the logical size.
                    let (width, height) = frame.renderer.size();
                    let logical = Vector2 {
                        x: width as f32 / scale,
                        y: height as f32 / scale,
                    };
                    if let Some(tree) = node::snapshot_in(&node, logical) {
                        frame.renderer.set_root(tree);
                    }
                    frame.renderer.set_scale(scale);

                    use wgpu::CurrentSurfaceTexture as Acquired;
                    let texture = match frame.surface.get_current_texture() {
                        Acquired::Success(texture) | Acquired::Suboptimal(texture) => texture,
                        // Outdated and Lost are handled by the resize arm; skip the rest.
                        _ => return,
                    };
                    frame.renderer.render_to_surface(&texture);
                    frame.queue.present(texture);
                    self.presented += 1;
                    if self.limit.is_some_and(|limit| self.presented >= limit) {
                        let (width, height) = frame.renderer.size();
                        println!(
                            "presented {} frame(s) at {width}x{height} {:?}",
                            self.presented,
                            frame.renderer.format()
                        );
                        event_loop.exit();
                        return;
                    }
                }
                _ => return,
            }
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
}
