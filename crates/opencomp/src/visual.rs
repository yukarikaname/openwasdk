//! `ContainerVisual` and `SpriteVisual`: the two leaves a tree is made of.
//!
//! Offset, size, opacity, visibility, clip, anchor point, scale, rotation and centre
//! point live on the shared node and reach the renderer. The transform matrix,
//! orientation, border mode, backface and composite mode are accepted and dropped:
//! a client that sets them gets a consistent tree, just not that part of the picture.

use std::sync::Mutex;

use crate::bindings::Microsoft::UI::Composition::{
    CompositionBrush, CompositionShadow, ContainerVisual, ICompositionColorBrush,
    ICompositionObject, ICompositionSurfaceBrush, IContainerVisual, IContainerVisual_Impl,
    ISpriteVisual, ISpriteVisual_Impl, IVisual, SpriteVisual, VisualCollection,
};
use crate::node::{self, Node, NodeRef};
use crate::object::prelude::*;
use crate::object::{composition_object, not_implemented, visual_trait};
use windows_core::{Interface, Result};

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    IContainerVisual,
    IVisual,
    IVisual2,
    IVisual3,
    IVisual4
)]
pub struct ContainerVisualObject {
    node: NodeRef,
    clip: Mutex<Option<CompositionClip>>,
}

#[windows_core::implement(
    ICompositionObject,
    ICompositionObject2,
    ICompositionObject4,
    ICompositionObject5,
    ISpriteVisual,
    ISpriteVisual2,
    IVisual,
    IVisual2,
    IVisual3,
    IVisual4
)]
pub struct SpriteVisualObject {
    node: NodeRef,
    clip: Mutex<Option<CompositionClip>>,
    brush: Mutex<Option<CompositionBrush>>,
}

composition_object!(ContainerVisualObject_Impl, node);
composition_object!(SpriteVisualObject_Impl, node);
visual_trait!(ContainerVisualObject_Impl);
visual_trait!(SpriteVisualObject_Impl);

impl IContainerVisual_Impl for ContainerVisualObject_Impl {
    fn Children(&self) -> Result<VisualCollection> {
        crate::collection::create(self.node.clone())
    }
}

impl ISpriteVisual2_Impl for SpriteVisualObject_Impl {
    fn Shadow(&self) -> Result<CompositionShadow> {
        not_implemented()
    }

    fn SetShadow(&self, _value: windows_core::Ref<CompositionShadow>) -> Result<()> {
        not_implemented()
    }
}

impl ISpriteVisual_Impl for SpriteVisualObject_Impl {
    fn Brush(&self) -> Result<CompositionBrush> {
        match self.brush.lock().unwrap().clone() {
            Some(brush) => Ok(brush),
            None => crate::brush::transparent(),
        }
    }

    fn SetBrush(&self, value: windows_core::Ref<CompositionBrush>) -> Result<()> {
        let brush = value.as_ref();
        let color = brush
            .and_then(|brush| brush.cast::<ICompositionColorBrush>().ok())
            .and_then(|brush| brush.Color().ok())
            .map_or([0.0; 4], crate::brush::to_rgba);
        let surface = brush
            .and_then(|brush| brush.cast::<ICompositionSurfaceBrush>().ok())
            .and_then(|brush| brush.Surface().ok())
            .and_then(|surface| crate::surface::describe(&surface));
        {
            let mut node = self.node.lock().unwrap();
            match surface {
                Some((id, width, height)) => {
                    // The bitmap is sampled as it is, so the tint has to leave it alone.
                    node.surface = Some(id);
                    node.color = [1.0; 4];
                    // A sprite with no size yet takes the surface's.
                    if node.size.x <= 0.0 || node.size.y <= 0.0 {
                        node.size = Vector2 {
                            x: width as f32,
                            y: height as f32,
                        };
                    }
                }
                None => {
                    node.surface = None;
                    node.color = color;
                }
            }
        }
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
