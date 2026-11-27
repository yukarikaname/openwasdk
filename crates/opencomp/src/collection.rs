//! `ContainerVisual.Children`: the edit surface of the tree.
//!
//! The collection holds the parent's node, not the parent object, so a tree stays valid even if
//! the client drops the `ContainerVisual` it got the collection from.

use std::sync::Arc;

use crate::bindings::Microsoft::UI::Composition::{
    ICompositionObject, ICompositionObject2, ICompositionObject4, ICompositionObject5,
    IVisualCollection, IVisualCollection_Impl, Visual, VisualCollection,
};
use crate::node::{self, NodeRef};
use crate::object::composition_object;
use windows_core::{Error, HRESULT, Interface, Result};

const E_INVALIDARG: HRESULT = HRESULT(0x8007_0057u32 as i32);

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    IVisualCollection
)]
pub struct VisualCollectionObject {
    parent: NodeRef,
}

composition_object!(VisualCollectionObject_Impl);

/// The node behind a `Visual` argument, refusing visuals made by someone else.
fn node_of(value: &windows_core::Ref<Visual>) -> Result<NodeRef> {
    value
        .as_ref()
        .and_then(node::resolve)
        .ok_or_else(|| Error::from_hresult(E_INVALIDARG))
}

/// Where `sibling` sits in `children`, if it is there at all.
fn position(children: &[NodeRef], sibling: &NodeRef) -> Result<usize> {
    children
        .iter()
        .position(|child| Arc::ptr_eq(child, sibling))
        .ok_or_else(|| Error::from_hresult(E_INVALIDARG))
}

impl IVisualCollection_Impl for VisualCollectionObject_Impl {
    fn Count(&self) -> Result<i32> {
        Ok(self.parent.lock().unwrap().children.len() as i32)
    }

    fn InsertAtTop(&self, newChild: windows_core::Ref<Visual>) -> Result<()> {
        let child = node_of(&newChild)?;
        let mut parent = self.parent.lock().unwrap();
        if !parent
            .children
            .iter()
            .any(|existing| Arc::ptr_eq(existing, &child))
        {
            parent.children.push(child);
        }
        Ok(())
    }

    fn InsertAtBottom(&self, newChild: windows_core::Ref<Visual>) -> Result<()> {
        let child = node_of(&newChild)?;
        let mut parent = self.parent.lock().unwrap();
        if !parent
            .children
            .iter()
            .any(|existing| Arc::ptr_eq(existing, &child))
        {
            parent.children.insert(0, child);
        }
        Ok(())
    }

    fn InsertAbove(
        &self,
        newChild: windows_core::Ref<Visual>,
        sibling: windows_core::Ref<Visual>,
    ) -> Result<()> {
        let child = node_of(&newChild)?;
        let sibling = node_of(&sibling)?;
        let mut parent = self.parent.lock().unwrap();
        let at = position(&parent.children, &sibling)? + 1;
        parent.children.insert(at, child);
        Ok(())
    }

    fn InsertBelow(
        &self,
        newChild: windows_core::Ref<Visual>,
        sibling: windows_core::Ref<Visual>,
    ) -> Result<()> {
        let child = node_of(&newChild)?;
        let sibling = node_of(&sibling)?;
        let mut parent = self.parent.lock().unwrap();
        let at = position(&parent.children, &sibling)?;
        parent.children.insert(at, child);
        Ok(())
    }

    fn Remove(&self, child: windows_core::Ref<Visual>) -> Result<()> {
        let child = node_of(&child)?;
        self.parent
            .lock()
            .unwrap()
            .children
            .retain(|existing| !Arc::ptr_eq(existing, &child));
        Ok(())
    }

    fn RemoveAll(&self) -> Result<()> {
        self.parent.lock().unwrap().children.clear();
        Ok(())
    }
}

pub fn create(parent: NodeRef) -> Result<VisualCollection> {
    let collection: IVisualCollection = VisualCollectionObject { parent }.into();
    collection.cast()
}
