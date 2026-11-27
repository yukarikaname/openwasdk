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
use windows_core::{Error, HRESULT, IInspectable, Interface, Ref, Result};
use windows_numerics::Vector2;

const E_INVALIDARG: HRESULT = HRESULT(0x8007_0057u32 as i32);
const E_POINTER: HRESULT = HRESULT(0x8000_4003u32 as i32);

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
    layout_direction: ContentLayoutDirection,
    rasterization_scale: f32,
    /// Tokens are handed out from one counter, so a token from one event never
    /// removes a handler registered on the other. The handlers themselves live on
    /// the object: a COM pointer is not `Send`, and this state is published by
    /// identity, which is.
    next_token: i64,
    automation_tokens: Vec<i64>,
    state_tokens: Vec<i64>,
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
            layout_direction: ContentLayoutDirection::LeftToRight,
            rasterization_scale: 1.0,
            next_token: 1,
            automation_tokens: Vec::new(),
            state_tokens: Vec::new(),
        }
    }

    fn token(&mut self) -> i64 {
        let token = self.next_token;
        self.next_token += 1;
        token
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
    app_data: Mutex<Option<IInspectable>>,
    automation_handlers: Mutex<
        Vec<(
            i64,
            TypedEventHandler<ContentIsland, ContentIslandAutomationProviderRequestedEventArgs>,
        )>,
    >,
    state_handlers: Mutex<
        Vec<(
            i64,
            TypedEventHandler<ContentIsland, ContentIslandStateChangedEventArgs>,
        )>,
    >,
}

impl IContentIsland_Impl for Island_Impl {
    fn ActualSize(&self) -> Result<Vector2> {
        Ok(self.state.lock().unwrap().size)
    }
    fn AppData(&self) -> Result<IInspectable> {
        match self.app_data.lock().unwrap().clone() {
            Some(value) => Ok(value),
            // Nullable in the ABI; the trait cannot say so.
            None => not_implemented(),
        }
    }
    fn SetAppData(&self, value: Ref<IInspectable>) -> Result<()> {
        *self.app_data.lock().unwrap() = value.cloned();
        Ok(())
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
        Ok(self.state.lock().unwrap().layout_direction)
    }
    fn RasterizationScale(&self) -> Result<f32> {
        Ok(self.state.lock().unwrap().rasterization_scale)
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
        handler: Ref<
            TypedEventHandler<ContentIsland, ContentIslandAutomationProviderRequestedEventArgs>,
        >,
    ) -> Result<i64> {
        let Some(handler) = handler.cloned() else {
            return Err(Error::from_hresult(E_POINTER));
        };
        let token = {
            let mut state = self.state.lock().unwrap();
            let token = state.token();
            state.automation_tokens.push(token);
            token
        };
        self.automation_handlers
            .lock()
            .unwrap()
            .push((token, handler));
        Ok(token)
    }
    fn RemoveAutomationProviderRequested(&self, token: i64) -> Result<()> {
        self.state
            .lock()
            .unwrap()
            .automation_tokens
            .retain(|existing| *existing != token);
        self.automation_handlers
            .lock()
            .unwrap()
            .retain(|(existing, _)| *existing != token);
        Ok(())
    }
    fn StateChanged(
        &self,
        handler: Ref<TypedEventHandler<ContentIsland, ContentIslandStateChangedEventArgs>>,
    ) -> Result<i64> {
        let Some(handler) = handler.cloned() else {
            return Err(Error::from_hresult(E_POINTER));
        };
        let token = {
            let mut state = self.state.lock().unwrap();
            let token = state.token();
            state.state_tokens.push(token);
            token
        };
        self.state_handlers.lock().unwrap().push((token, handler));
        Ok(token)
    }
    fn RemoveStateChanged(&self, token: i64) -> Result<()> {
        self.state
            .lock()
            .unwrap()
            .state_tokens
            .retain(|existing| *existing != token);
        self.state_handlers
            .lock()
            .unwrap()
            .retain(|(existing, _)| *existing != token);
        Ok(())
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
        app_data: Mutex::new(None),
        automation_handlers: Mutex::new(Vec::new()),
        state_handlers: Mutex::new(Vec::new()),
    }
    .into();
    crate::identity::publish(&island, &state);
    island.cast()
}

pub(crate) fn island_statics() -> Result<IContentIslandStatics> {
    Ok(IslandStatics.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::Microsoft::UI::Composition::ICompositor;
    use windows_core::Interface;

    #[test]
    fn an_island_keeps_app_data_and_event_tokens() {
        let compositor: ICompositor = crate::compositor::instance().unwrap().cast().unwrap();
        let root = compositor.CreateContainerVisual().unwrap();
        let visual: Visual = root.cast().unwrap();
        let island = island_from_visual(&visual).unwrap();
        let api: IContentIsland = island.cast().unwrap();

        let payload: IInspectable = root.cast().unwrap();
        api.SetAppData(&payload).unwrap();
        let stored = api.AppData().unwrap();
        assert_eq!(Interface::as_raw(&stored), Interface::as_raw(&payload));
        assert_eq!(
            api.LayoutDirection().unwrap(),
            ContentLayoutDirection::LeftToRight
        );
        assert_eq!(api.RasterizationScale().unwrap(), 1.0);

        // The client-facing helpers wrap the event as an `EventRevoker`; the token
        // an ABI caller sees is only reachable through the vtable.
        let handler = TypedEventHandler::<
            ContentIsland,
            ContentIslandAutomationProviderRequestedEventArgs,
        >::new(|_, _| {});
        let vtable = Interface::vtable(&api);
        let mut token = 0i64;
        unsafe {
            (vtable.AutomationProviderRequested)(
                Interface::as_raw(&api),
                handler.as_raw(),
                &mut token,
            )
            .ok()
            .unwrap();
        }
        assert!(token > 0);
        assert_eq!(
            shared(&island)
                .unwrap()
                .lock()
                .unwrap()
                .automation_tokens
                .len(),
            1
        );
        unsafe {
            (vtable.RemoveAutomationProviderRequested)(Interface::as_raw(&api), token)
                .ok()
                .unwrap();
        }
        assert!(
            shared(&island)
                .unwrap()
                .lock()
                .unwrap()
                .automation_tokens
                .is_empty()
        );

        let handler =
            TypedEventHandler::<ContentIsland, ContentIslandStateChangedEventArgs>::new(|_, _| {});
        let mut token = 0i64;
        unsafe {
            (vtable.StateChanged)(Interface::as_raw(&api), handler.as_raw(), &mut token)
                .ok()
                .unwrap();
        }
        assert!(token > 0);
        assert_eq!(
            shared(&island).unwrap().lock().unwrap().state_tokens.len(),
            1
        );
        unsafe {
            (vtable.RemoveStateChanged)(Interface::as_raw(&api), token)
                .ok()
                .unwrap();
        }
        assert!(
            shared(&island)
                .unwrap()
                .lock()
                .unwrap()
                .state_tokens
                .is_empty()
        );
    }

    /// A statics object is reached only through `QueryInterface` and then called through
    /// its vtable, so the two pointers that has to work on are asserted here: a broken
    /// object shows up as a fault inside the first `Create`, with nothing to say why.
    #[test]
    fn a_statics_object_points_at_a_vtable() {
        let statics = island_statics().expect("statics");
        let raw = Interface::as_raw(&statics) as usize;
        assert_eq!(raw % core::mem::align_of::<usize>(), 0, "object {raw:#x}");
        // Safety: the pointer is aligned, and reading one word of a live object cannot fault.
        let table = unsafe { (raw as *const usize).read_unaligned() };
        assert_ne!(table, 0, "no vtable");
        assert_eq!(
            table % core::mem::align_of::<usize>(),
            0,
            "vtable {table:#x}"
        );
    }
}
