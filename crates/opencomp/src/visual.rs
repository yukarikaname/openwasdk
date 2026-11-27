//! `ContainerVisual` and `SpriteVisual`: the two leaves a tree is made of.
//!
//! The properties the renderer understands live on the shared node; the rest of `IVisual`
//! (rotation, transforms, anchors) is accepted by the ABI and ignored, which is all a first frame
//! needs. A client that sets them gets a consistent tree, just not the transform.

use std::sync::Mutex;

use crate::bindings::Microsoft::UI::Composition::{
    CompositionBrush, ContainerVisual, ICompositionColorBrush, ICompositionObject,
    IContainerVisual, IContainerVisual_Impl, ISpriteVisual, ISpriteVisual_Impl, IVisual,
    SpriteVisual, VisualCollection,
};
use crate::node::{self, Node, NodeRef};
use crate::object::E_FAIL;
use crate::object::prelude::*;
use crate::object::{composition_object, visual_trait};
use windows_core::{Error, Interface, Result};

#[windows_core::implement(ICompositionObject, IContainerVisual, IVisual)]
pub struct ContainerVisualObject {
    node: NodeRef,
    clip: Mutex<Option<CompositionClip>>,
}

#[windows_core::implement(ICompositionObject, ISpriteVisual, IVisual)]
pub struct SpriteVisualObject {
    node: NodeRef,
    clip: Mutex<Option<CompositionClip>>,
    brush: Mutex<Option<CompositionBrush>>,
}

composition_object!(ContainerVisualObject_Impl);
composition_object!(SpriteVisualObject_Impl);
visual_trait!(ContainerVisualObject_Impl);
visual_trait!(SpriteVisualObject_Impl);

impl IContainerVisual_Impl for ContainerVisualObject_Impl {
    fn Children(&self) -> Result<VisualCollection> {
        crate::collection::create(self.node.clone())
    }
}

impl ISpriteVisual_Impl for SpriteVisualObject_Impl {
    fn Brush(&self) -> Result<CompositionBrush> {
        match self.brush.lock().unwrap().clone() {
            Some(brush) => Ok(brush),
            None => Err(Error::from_hresult(E_FAIL)),
        }
    }

    fn SetBrush(&self, value: windows_core::Ref<CompositionBrush>) -> Result<()> {
        let color = value
            .as_ref()
            .and_then(|brush| brush.cast::<ICompositionColorBrush>().ok())
            .and_then(|brush| brush.Color().ok())
            .map_or([0.0; 4], crate::brush::to_rgba);
        self.node.lock().unwrap().color = color;
        *self.brush.lock().unwrap() = value.cloned();
        Ok(())
    }
}

/// A `ContainerVisual` with no children.
pub fn container() -> Result<ContainerVisual> {
    let node = Node::new();
    let object: IContainerVisual = ContainerVisualObject {
        node: node.clone(),
        clip: Mutex::new(None),
    }
    .into();
    node::publish(&object, &node);
    object.cast()
}

/// A `SpriteVisual` with no brush, which draws nothing until one is set.
pub fn sprite() -> Result<SpriteVisual> {
    let node = Node::new();
    let object: ISpriteVisual = SpriteVisualObject {
        node: node.clone(),
        clip: Mutex::new(None),
        brush: Mutex::new(None),
    }
    .into();
    node::publish(&object, &node);
    object.cast()
}
