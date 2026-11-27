//! `Microsoft.UI.Content` hosting: a `ContentIsland` and the child site bridge that owns it.
//!
//! A composition-only client wraps a visual tree in an island and connects it to a site bridge.
//! This module keeps the graph and hands the root node to the host for the renderer snapshot.
//!
//! The shipped package has no Content DLL of its own: `Microsoft.ui.xaml.dll` carries the
//! activation strings for `ContentIsland` and `DesktopChildSiteBridge` and exports
//! `DllGetActivationFactory`. Serving them from `opencomp` is a deliberate choice for our own
//! composition-only host; a faithful mirror would put them in `openxaml`. The class to DLL
//! mapping lives only in `lib.rs`, so it can move. D3D, swapchains and the message loop are
//! out of scope: the bridge records the window id and rectangle and presents no frame.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::bindings::Microsoft::UI::Composition::{Compositor, Visual};
use crate::bindings::Microsoft::UI::Content::*;
use crate::bindings::Microsoft::UI::Dispatching::DispatcherQueue;
use crate::bindings::Microsoft::UI::WindowId;
use crate::bindings::Windows::Foundation::Collections::IPropertySet;
use crate::bindings::Windows::Foundation::{IClosable, IClosable_Impl, TypedEventHandler};
use crate::bindings::Windows::Graphics::RectInt32;
use crate::node::NodeRef;
use crate::object::not_implemented;
use windows_core::{Error, HRESULT, Interface, Ref, Result};
use windows_numerics::Vector2;

const E_INVALIDARG: HRESULT = HRESULT(0x8007_0057u32 as i32);

/// State shared by the island object and whichever bridge is connected to it.
struct IslandState {
    node: NodeRef,
    size: Vector2,
    connected: bool,
    island_enabled: bool,
    island_visible: bool,
    site_enabled: bool,
    site_visible: bool,
    hit_test_visible: bool,
}

impl IslandState {
    fn new(node: NodeRef) -> Self {
        Self {
            node,
            size: Vector2::default(),
            connected: false,
            island_enabled: true,
            island_visible: true,
            site_enabled: false,
            site_visible: false,
            hit_test_visible: true,
        }
    }
}

type SharedIsland = Arc<Mutex<IslandState>>;

/// The node behind an island, for a host that wants to snapshot it.
pub fn root_node(island: &ContentIsland) -> Option<NodeRef> {
    shared(island).map(|state| state.lock().unwrap().node.clone())
}

fn shared(island: &ContentIsland) -> Option<SharedIsland> {
    crate::identity::lookup::<_, Mutex<IslandState>>(island)
}

#[windows_core::implement(IContentIsland)]
struct Island {
    state: SharedIsland,
    id: u64,
}

impl IContentIsland_Impl for Island_Impl {
    fn ActualSize(&self) -> Result<Vector2> {
        Ok(self.state.lock().unwrap().size)
    }
    fn AppData(&self) -> Result<windows_core::IInspectable> {
        not_implemented()
    }
    fn SetAppData(&self, _value: Ref<windows_core::IInspectable>) -> Result<()> {
        not_implemented()
    }
    fn CoordinateConverter(&self) -> Result<ContentCoordinateConverter> {
        not_implemented()
    }
    fn CustomProperties(&self) -> Result<IPropertySet> {
        not_implemented()
    }
    fn DispatcherQueue(&self) -> Result<DispatcherQueue> {
        not_implemented()
    }
    fn Environment(&self) -> Result<ContentIslandEnvironment> {
        not_implemented()
    }
    fn Id(&self) -> Result<u64> {
        Ok(self.id)
    }
    fn IsConnected(&self) -> Result<bool> {
        Ok(self.state.lock().unwrap().connected)
    }
    fn IsHitTestVisibleWhenTransparent(&self) -> Result<bool> {
        Ok(self.state.lock().unwrap().hit_test_visible)
    }
    fn SetIsHitTestVisibleWhenTransparent(&self, value: bool) -> Result<()> {
        self.state.lock().unwrap().hit_test_visible = value;
        Ok(())
    }
    fn IsIslandEnabled(&self) -> Result<bool> {
        Ok(self.state.lock().unwrap().island_enabled)
    }
    fn SetIsIslandEnabled(&self, value: bool) -> Result<()> {
        self.state.lock().unwrap().island_enabled = value;
        Ok(())
    }
    fn IsIslandVisible(&self) -> Result<bool> {
        Ok(self.state.lock().unwrap().island_visible)
    }
    fn SetIsIslandVisible(&self, value: bool) -> Result<()> {
        self.state.lock().unwrap().island_visible = value;
        Ok(())
    }
    fn IsSiteEnabled(&self) -> Result<bool> {
        Ok(self.state.lock().unwrap().site_enabled)
    }
    fn IsSiteVisible(&self) -> Result<bool> {
        Ok(self.state.lock().unwrap().site_visible)
    }
    fn LayoutDirection(&self) -> Result<ContentLayoutDirection> {
        not_implemented()
    }
    fn RasterizationScale(&self) -> Result<f32> {
        not_implemented()
    }
    fn GetAutomationHostProvider(&self) -> Result<windows_core::IInspectable> {
        not_implemented()
    }
    fn GetStateChangeDeferral(&self) -> Result<ContentDeferral> {
        not_implemented()
    }
    fn RequestSize(&self, _size: &Vector2) -> Result<()> {
        // The site decides the size; without one there is nothing to ask.
        not_implemented()
    }
    fn AutomationProviderRequested(
        &self,
        _handler: Ref<
            TypedEventHandler<ContentIsland, ContentIslandAutomationProviderRequestedEventArgs>,
        >,
    ) -> Result<i64> {
        not_implemented()
    }
    fn RemoveAutomationProviderRequested(&self, _token: i64) -> Result<()> {
        not_implemented()
    }
    fn StateChanged(
        &self,
        _handler: Ref<TypedEventHandler<ContentIsland, ContentIslandStateChangedEventArgs>>,
    ) -> Result<i64> {
        not_implemented()
    }
    fn RemoveStateChanged(&self, _token: i64) -> Result<()> {
        not_implemented()
    }
}

#[windows_core::implement(IContentIslandStatics)]
struct IslandStatics;

/// Wrap a visual this DLL made in an island. A visual from elsewhere has no node to publish.
fn island_from_visual(root: &Visual) -> Result<ContentIsland> {
    let node = crate::node::resolve(root).ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
    create_island(node)
}

impl IContentIslandStatics_Impl for IslandStatics_Impl {
    fn Create(&self, root: Ref<Visual>) -> Result<ContentIsland> {
        let root = root
            .as_ref()
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        island_from_visual(root)
    }
    fn FindAllForCompositor(
        &self,
        _compositor: Ref<Compositor>,
    ) -> Result<windows_core::Array<ContentIsland>> {
        not_implemented()
    }
    fn FindAllForCurrentThread(&self) -> Result<windows_core::Array<ContentIsland>> {
        not_implemented()
    }
    fn GetByVisual(&self, _child: Ref<Visual>) -> Result<ContentIsland> {
        not_implemented()
    }
    fn GetFromId(&self, _id: u64) -> Result<ContentIsland> {
        not_implemented()
    }
}

/// Create an island around a root node. The state is published by identity so that a bridge
/// and the host can find it from the COM object.
fn create_island(node: NodeRef) -> Result<ContentIsland> {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let state: SharedIsland = Arc::new(Mutex::new(IslandState::new(node)));
    let island: IContentIsland = Island {
        state: state.clone(),
        id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
    }
    .into();
    crate::identity::publish(&island, &state);
    island.cast()
}

pub(crate) fn island_statics() -> Result<IContentIslandStatics> {
    Ok(IslandStatics.into())
}

struct BridgeState {
    enabled: bool,
    visible: bool,
    rect: RectInt32,
    scale: f32,
    resize_policy: ContentSizePolicy,
    island: Option<SharedIsland>,
}

impl BridgeState {
    fn new() -> Self {
        Self {
            enabled: true,
            visible: false,
            rect: RectInt32::default(),
            scale: 1.0,
            resize_policy: ContentSizePolicy::None,
            island: None,
        }
    }
}

#[windows_core::implement(
    IClosable,
    IContentSiteBridge,
    IDesktopSiteBridge,
    IDesktopChildSiteBridge
)]
struct Bridge {
    window: WindowId,
    state: Mutex<BridgeState>,
}

impl IClosable_Impl for Bridge_Impl {
    fn Close(&self) -> Result<()> {
        let mut bridge = self.state.lock().unwrap();
        if let Some(island) = bridge.island.take() {
            let mut island = island.lock().unwrap();
            island.connected = false;
            island.site_enabled = false;
            island.site_visible = false;
        }
        bridge.enabled = false;
        bridge.visible = false;
        Ok(())
    }
}

impl IContentSiteBridge_Impl for Bridge_Impl {
    fn DispatcherQueue(&self) -> Result<DispatcherQueue> {
        not_implemented()
    }
    fn LayoutDirectionOverride(
        &self,
    ) -> Result<windows_reference::IReference<ContentLayoutDirection>> {
        not_implemented()
    }
    fn SetLayoutDirectionOverride(
        &self,
        _value: Ref<windows_reference::IReference<ContentLayoutDirection>>,
    ) -> Result<()> {
        not_implemented()
    }
    fn OverrideScale(&self) -> Result<f32> {
        Ok(self.state.lock().unwrap().scale)
    }
    fn SetOverrideScale(&self, value: f32) -> Result<()> {
        self.state.lock().unwrap().scale = value;
        Ok(())
    }
}

impl IDesktopSiteBridge_Impl for Bridge_Impl {
    fn IsEnabled(&self) -> Result<bool> {
        Ok(self.state.lock().unwrap().enabled)
    }
    fn IsVisible(&self) -> Result<bool> {
        Ok(self.state.lock().unwrap().visible)
    }
    fn WindowId(&self) -> Result<WindowId> {
        Ok(self.window)
    }
    fn Connect(&self, content: Ref<ContentIsland>) -> Result<()> {
        let island = content
            .as_ref()
            .and_then(shared)
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        let mut bridge = self.state.lock().unwrap();
        if let Some(previous) = bridge.island.replace(island.clone())
            && !Arc::ptr_eq(&previous, &island)
        {
            previous.lock().unwrap().connected = false;
        }
        let mut state = island.lock().unwrap();
        state.connected = true;
        state.site_enabled = bridge.enabled;
        state.site_visible = bridge.visible;
        Ok(())
    }
    fn Disable(&self) -> Result<()> {
        let mut bridge = self.state.lock().unwrap();
        bridge.enabled = false;
        if let Some(island) = &bridge.island {
            island.lock().unwrap().site_enabled = false;
        }
        Ok(())
    }
    fn Enable(&self) -> Result<()> {
        let mut bridge = self.state.lock().unwrap();
        bridge.enabled = true;
        if let Some(island) = &bridge.island {
            island.lock().unwrap().site_enabled = true;
        }
        Ok(())
    }
    fn Hide(&self) -> Result<()> {
        let mut bridge = self.state.lock().unwrap();
        bridge.visible = false;
        if let Some(island) = &bridge.island {
            island.lock().unwrap().site_visible = false;
        }
        Ok(())
    }
    fn MoveAndResize(&self, rect: &RectInt32) -> Result<()> {
        let mut bridge = self.state.lock().unwrap();
        bridge.rect = *rect;
        if let Some(island) = &bridge.island {
            let mut state = island.lock().unwrap();
            state.size = Vector2 {
                x: rect.width.max(0) as f32,
                y: rect.height.max(0) as f32,
            };
        }
        Ok(())
    }
    fn MoveInZOrderAtBottom(&self) -> Result<()> {
        not_implemented()
    }
    fn MoveInZOrderAtTop(&self) -> Result<()> {
        not_implemented()
    }
    fn MoveInZOrderBelow(&self, _windowId: &WindowId) -> Result<()> {
        not_implemented()
    }
    fn Show(&self) -> Result<()> {
        let mut bridge = self.state.lock().unwrap();
        bridge.visible = true;
        if let Some(island) = &bridge.island {
            island.lock().unwrap().site_visible = true;
        }
        Ok(())
    }
}

impl IDesktopChildSiteBridge_Impl for Bridge_Impl {
    fn ResizePolicy(&self) -> Result<ContentSizePolicy> {
        Ok(self.state.lock().unwrap().resize_policy)
    }
    fn SetResizePolicy(&self, value: ContentSizePolicy) -> Result<()> {
        self.state.lock().unwrap().resize_policy = value;
        Ok(())
    }
    fn SiteView(&self) -> Result<ContentSiteView> {
        not_implemented()
    }
}

#[windows_core::implement(IDesktopChildSiteBridgeStatics)]
struct BridgeStatics;

impl IDesktopChildSiteBridgeStatics_Impl for BridgeStatics_Impl {
    fn Create(
        &self,
        compositor: Ref<Compositor>,
        parentWindowId: &WindowId,
    ) -> Result<DesktopChildSiteBridge> {
        // The compositor is not used for the snapshot handoff, but a client that passes
        // nothing did not mean to create a working bridge.
        let _compositor = compositor
            .as_ref()
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        create_bridge(*parentWindowId)
    }
}

fn create_bridge(window: WindowId) -> Result<DesktopChildSiteBridge> {
    let bridge: IDesktopChildSiteBridge = Bridge {
        window,
        state: Mutex::new(BridgeState::new()),
    }
    .into();
    bridge.cast()
}

pub(crate) fn bridge_statics() -> Result<IDesktopChildSiteBridgeStatics> {
    Ok(BridgeStatics.into())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::bindings::Microsoft::UI::Composition::{ICompositor, IVisual2};

    fn compositor() -> ICompositor {
        crate::compositor::instance().unwrap().cast().unwrap()
    }

    fn island_with_relative_root(compositor: &ICompositor) -> (ContentIsland, NodeRef) {
        let container = compositor.CreateContainerVisual().unwrap();
        let visual: IVisual2 = container.cast().unwrap();
        visual
            .SetRelativeSizeAdjustment(Vector2 { x: 1.0, y: 1.0 })
            .unwrap();
        let root: Visual = container.cast().unwrap();
        let node = crate::node::resolve(&root).unwrap();
        let island = island_from_visual(&root).unwrap();
        (island, node)
    }

    fn bridge(window: u64) -> DesktopChildSiteBridge {
        create_bridge(WindowId { value: window }).unwrap()
    }

    #[test]
    fn a_connected_island_keeps_the_root_it_was_given() {
        let compositor = compositor();
        let (island, node) = island_with_relative_root(&compositor);
        let bridge = bridge(0x1234);
        let site: IDesktopSiteBridge = bridge.cast().unwrap();
        site.Connect(&island).unwrap();

        assert_eq!(site.WindowId().unwrap().value, 0x1234);
        assert!(site.IsEnabled().unwrap());
        let island_state: IContentIsland = island.cast().unwrap();
        assert!(island_state.IsConnected().unwrap());
        assert!(Arc::ptr_eq(&root_node(&island).unwrap(), &node));
    }

    #[test]
    fn a_relative_root_fills_the_bridge_rectangle() {
        let compositor = compositor();
        let (island, _) = island_with_relative_root(&compositor);
        let bridge = bridge(0x1234);
        let site: IDesktopSiteBridge = bridge.cast().unwrap();
        site.Connect(&island).unwrap();
        site.Show().unwrap();
        site.MoveAndResize(RectInt32 {
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        })
        .unwrap();

        let island_state: IContentIsland = island.cast().unwrap();
        let size = island_state.ActualSize().unwrap();
        assert_eq!((size.x, size.y), (800.0, 600.0));
        assert!(island_state.IsSiteVisible().unwrap());

        let node = root_node(&island).unwrap();
        let tree = crate::node::snapshot_in(&node, size).unwrap();
        assert_eq!(tree.size, [800.0, 600.0]);
    }

    #[test]
    fn hiding_the_bridge_hides_the_site() {
        let compositor = compositor();
        let (island, _) = island_with_relative_root(&compositor);
        let bridge = bridge(0x1234);
        let site: IDesktopSiteBridge = bridge.cast().unwrap();
        site.Connect(&island).unwrap();
        site.Show().unwrap();
        site.Hide().unwrap();

        let island_state: IContentIsland = island.cast().unwrap();
        assert!(!site.IsVisible().unwrap());
        assert!(!island_state.IsSiteVisible().unwrap());
    }
}
