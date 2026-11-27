//! `DesktopChildSiteBridge`: the site that owns an island and its window.

use std::sync::{Arc, Mutex};

use crate::bindings::Microsoft::UI::Composition::Compositor;
use crate::bindings::Microsoft::UI::Content::*;
use crate::bindings::Microsoft::UI::Dispatching::DispatcherQueue;
use crate::bindings::Microsoft::UI::WindowId;
use crate::bindings::Windows::Foundation::{IClosable, IClosable_Impl};
use crate::bindings::Windows::Graphics::RectInt32;
use crate::clock::Clock;
use crate::object::not_implemented;
use windows_core::{Error, HRESULT, Interface, Ref, Result};
use windows_numerics::Vector2;

use super::island::{SharedIsland, shared};

const E_INVALIDARG: HRESULT = HRESULT(0x8007_0057u32 as i32);

struct BridgeState {
    enabled: bool,
    visible: bool,
    rect: RectInt32,
    scale: f32,
    resize_policy: ContentSizePolicy,
    layout_override: Option<ContentLayoutDirection>,
    island: Option<SharedIsland>,
    /// The compositor's clock; the frame loop is the only driver of it.
    clock: Arc<Clock>,
    /// The frame loop, started on the first `Show` and stopped by `Hide` or `Close`.
    presenter: Option<crate::present::Window>,
}

impl BridgeState {
    fn new(clock: Arc<Clock>) -> Self {
        Self {
            enabled: true,
            visible: false,
            rect: RectInt32::default(),
            scale: 1.0,
            resize_policy: ContentSizePolicy::None,
            layout_override: None,
            island: None,
            clock,
            presenter: None,
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
        if let Some(mut presenter) = bridge.presenter.take() {
            presenter.stop();
        }
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
        match self.state.lock().unwrap().layout_override {
            // A stock reference boxes the value; the island reads the plain enum.
            Some(value) => Ok(windows_reference::IReference::from(value)),
            // Nullable in the ABI; the trait cannot say so.
            None => not_implemented(),
        }
    }
    fn SetLayoutDirectionOverride(
        &self,
        value: Ref<windows_reference::IReference<ContentLayoutDirection>>,
    ) -> Result<()> {
        let direction = match value.as_ref() {
            Some(reference) => Some(reference.Value()?),
            None => None,
        };
        self.state.lock().unwrap().layout_override = direction;
        Ok(())
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
        if let Some(mut presenter) = bridge.presenter.take() {
            presenter.stop();
        }
        bridge.visible = false;
        if let Some(island) = &bridge.island {
            island.lock().unwrap().site_visible = false;
        }
        Ok(())
    }
    fn MoveAndResize(&self, rect: &RectInt32) -> Result<()> {
        let mut bridge = self.state.lock().unwrap();
        bridge.rect = *rect;
        if let Some(presenter) = &bridge.presenter {
            presenter.resize(rect.width.max(0) as u32, rect.height.max(0) as u32);
        }
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
        if bridge.presenter.is_none() {
            start_presenting(&mut bridge, self.window);
        }
        Ok(())
    }
}

/// Start the frame loop for a bridge that has an island and a size.
///
/// A bridge with neither stays silent: `Show` is not where a client expects an
/// error, and a missing device is not something the client can repair.
fn start_presenting(bridge: &mut BridgeState, window: WindowId) {
    let Some(island) = bridge.island.as_ref() else {
        return;
    };
    let (root, size) = {
        let island = island.lock().unwrap();
        (
            island.node.clone(),
            (
                bridge.rect.width.max(0) as u32,
                bridge.rect.height.max(0) as u32,
            ),
        )
    };
    if size.0 == 0 || size.1 == 0 || window.value == 0 {
        return;
    }
    match crate::present::Window::start(root, window.value, size, bridge.clock.clone()) {
        Some(presenter) => bridge.presenter = Some(presenter),
        None => eprintln!("opencomp: no device for this window; nothing will be presented"),
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
        // The bridge presents the compositor's tree, so it has to drive that
        // compositor's clock. A foreign compositor has no clock this DLL can advance.
        let compositor = compositor
            .as_ref()
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        let clock = crate::compositor::clock_of(compositor)
            .ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        create_bridge(*parentWindowId, clock)
    }
}

fn create_bridge(window: WindowId, clock: Arc<Clock>) -> Result<DesktopChildSiteBridge> {
    let bridge: IDesktopChildSiteBridge = Bridge {
        window,
        state: Mutex::new(BridgeState::new(clock)),
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
    use crate::bindings::Microsoft::UI::Composition::Visual;
    use crate::bindings::Microsoft::UI::Composition::{ICompositor, IVisual2};
    use crate::content::island::{island_from_visual, root_node};
    use crate::node::NodeRef;

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
        create_bridge(WindowId { value: window }, Clock::new()).unwrap()
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
    fn a_layout_direction_override_round_trips() {
        let bridge = bridge(0x1234);
        let site: IContentSiteBridge = bridge.cast().unwrap();
        // Unset, the property is nullable in the ABI and the trait cannot say so.
        assert!(site.LayoutDirectionOverride().is_err());

        // The client-facing setter boxes the enum into the `IReference` the ABI takes.
        site.SetLayoutDirectionOverride(Some(ContentLayoutDirection::RightToLeft))
            .unwrap();
        assert_eq!(
            site.LayoutDirectionOverride().unwrap(),
            ContentLayoutDirection::RightToLeft
        );

        site.SetLayoutDirectionOverride(None).unwrap();
        assert!(site.LayoutDirectionOverride().is_err());
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
