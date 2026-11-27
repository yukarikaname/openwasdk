//! `ContentIsland`: the root visual, its size and whether a site has it.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::bindings::Microsoft::UI::Composition::{Compositor, Visual};
use crate::bindings::Microsoft::UI::Content::*;
use crate::bindings::Microsoft::UI::Dispatching::DispatcherQueue;
use crate::bindings::Windows::Foundation::Collections::IPropertySet;
use crate::bindings::Windows::Foundation::TypedEventHandler;
use crate::node::NodeRef;
use crate::object::not_implemented;
use windows_core::{Error, HRESULT, Interface, Ref, Result};
use windows_numerics::Vector2;

const E_INVALIDARG: HRESULT = HRESULT(0x8007_0057u32 as i32);

/// State shared by the island object and whichever bridge is connected to it.
pub(crate) struct IslandState {
    pub(crate) node: NodeRef,
    pub(crate) size: Vector2,
    pub(crate) connected: bool,
    pub(crate) island_enabled: bool,
    pub(crate) island_visible: bool,
    pub(crate) site_enabled: bool,
    pub(crate) site_visible: bool,
    pub(crate) hit_test_visible: bool,
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

pub(crate) type SharedIsland = Arc<Mutex<IslandState>>;

/// The node behind an island, for a host that wants to snapshot it.
pub fn root_node(island: &ContentIsland) -> Option<NodeRef> {
    shared(island).map(|state| state.lock().unwrap().node.clone())
}

pub(crate) fn shared(island: &ContentIsland) -> Option<SharedIsland> {
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
pub(crate) fn island_from_visual(root: &Visual) -> Result<ContentIsland> {
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
