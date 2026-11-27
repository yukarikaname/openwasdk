//! The state behind every WinRT visual, and the snapshot the renderer consumes.
//!
//! A `Visual` in the composition API is a COM object with children that can be edited in any
//! order. The renderer wants a tree it can walk. Both views share one node: the COM object keeps
//! the node alive, the parent keeps its children as nodes, and each frame walks the nodes.
//!
//! Nodes are also published here by identity, because `VisualCollection::InsertAtTop` receives a
//! COM pointer and has to find the node behind it. The table holds weak references, so a visual
//! that dies leaves nothing behind.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use opencomp_core::{Keyframes, Visual as CoreVisual};
use windows_core::Interface;
use windows_numerics::{Vector2, Vector3};

/// A node in the live tree. Shared between the COM object, its parent and any snapshot in flight.
pub type NodeRef = Arc<Mutex<Node>>;

pub struct Node {
    pub offset: Vector3,
    pub size: Vector2,
    pub opacity: f32,
    /// The point of the visual that `offset` places, as a fraction of its size.
    pub anchor: Vector2,
    /// Scale and rotation about `center`, both in the node's own coordinates.
    pub scale: Vector3,
    pub rotation: f32,
    pub center: Vector3,
    pub visible: bool,
    pub hit_test_visible: bool,
    pub color: [f32; 4],
    /// Left, top, right, bottom insets of an `InsetClip`, if one is attached.
    pub insets: Option<[f32; 4]>,
    /// Fraction of the parent's size, from `IVisual2`. A root visual uses this to fill its host.
    pub relative_size: Option<Vector2>,
    pub relative_offset: Option<Vector3>,
    pub animations: Vec<Running>,
    pub children: Vec<NodeRef>,
}

/// The properties an animation can drive. The names are the ones the ABI passes
/// to `ICompositionObject.StartAnimation`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Property {
    Offset,
    Size,
    Opacity,
    Color,
}

impl Property {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "Offset" => Some(Self::Offset),
            "Size" => Some(Self::Size),
            "Opacity" => Some(Self::Opacity),
            "Color" => Some(Self::Color),
            _ => None,
        }
    }
}

pub struct Running {
    pub property: Property,
    pub keyframes: Keyframes,
    pub started: Instant,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            offset: Vector3::default(),
            size: Vector2::default(),
            opacity: 1.0,
            anchor: Vector2::default(),
            scale: Vector3 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            rotation: 0.0,
            center: Vector3::default(),
            visible: true,
            hit_test_visible: true,
            color: [0.0; 4],
            insets: None,
            relative_size: None,
            relative_offset: None,
            animations: Vec::new(),
            children: Vec::new(),
        }
    }
}

impl Node {
    pub fn new() -> NodeRef {
        Arc::new(Mutex::new(Self::default()))
    }

    /// The animated value of each property at this instant, in draw order.
    fn animation_values(&self) -> impl Iterator<Item = (Property, [f32; 4])> + '_ {
        self.animations.iter().filter_map(|running| {
            let time = running.started.elapsed().as_secs_f32();
            running
                .keyframes
                .sample(time)
                .map(|value| (running.property, value))
        })
    }

    fn resolved_size(&self, parent: Vector2) -> Vector2 {
        match self.relative_size {
            Some(relative) => Vector2 {
                x: parent.x * relative.x,
                y: parent.y * relative.y,
            },
            None => self.size,
        }
    }
}

/// Copy the tree into the renderer's form, resolving relative sizes against nothing.
pub fn snapshot(node: &NodeRef) -> Option<CoreVisual> {
    snapshot_in(node, Vector2::default())
}

/// The same, with `parent` as the size a relative root resolves against.
pub fn snapshot_in(node: &NodeRef, parent: Vector2) -> Option<CoreVisual> {
    snapshot_at(node, parent, 1.0, 0)
}

fn snapshot_at(
    node: &NodeRef,
    parent: Vector2,
    parent_opacity: f32,
    depth: u32,
) -> Option<CoreVisual> {
    let (mut visual, size, children) = {
        let node = node.lock().unwrap();
        if !node.visible {
            return None;
        }
        let mut size = node.resolved_size(parent);
        let mut offset = match node.relative_offset {
            Some(relative) => [parent.x * relative.x, parent.y * relative.y],
            None => [node.offset.x, node.offset.y],
        };
        // The anchor is the point of the visual that `offset` places, so a centred anchor
        // moves the drawn rectangle back by half its size.
        offset[0] -= node.anchor.x * size.x;
        offset[1] -= node.anchor.y * size.y;
        let mut opacity = (parent_opacity * node.opacity).clamp(0.0, 1.0);
        let mut color = node.color;
        for (property, value) in node.animation_values() {
            match property {
                Property::Offset => offset = [value[0], value[1]],
                Property::Size => {
                    size = Vector2 {
                        x: value[0],
                        y: value[1],
                    }
                }
                Property::Opacity => opacity = value[0].clamp(0.0, 1.0),
                Property::Color => color = value,
            }
        }
        let visual = CoreVisual {
            offset,
            size: [size.x, size.y],
            color,
            opacity,
            scale: [node.scale.x, node.scale.y],
            rotation: node.rotation,
            center: [node.center.x, node.center.y],
            clip: node.insets.map(|insets| clip_of(size, insets)),
            children: Vec::new(),
        };
        // A visual inserted into its own subtree would recurse until the stack runs out; the
        // bound keeps such a tree renderable instead of fatal.
        let children = if depth < 64 {
            node.children.clone()
        } else {
            Vec::new()
        };
        (visual, size, children)
    };
    for child in &children {
        if let Some(child) = snapshot_at(child, size, visual.opacity, depth + 1) {
            visual.children.push(child);
        }
    }
    Some(visual)
}

/// Start an animation, replacing one already running on the same property.
pub fn start_animation(node: &NodeRef, property: Property, keyframes: Keyframes) {
    let mut node = node.lock().unwrap();
    node.animations
        .retain(|running| running.property != property);
    node.animations.push(Running {
        property,
        keyframes,
        started: Instant::now(),
    });
}

/// Stop an animation and leave the property where it last was.
pub fn stop_animation(node: &NodeRef, property: Property) {
    node.lock()
        .unwrap()
        .animations
        .retain(|running| running.property != property);
}

/// The clip rectangle an `InsetClip` produces inside a visual of `size`.
fn clip_of(size: Vector2, [left, top, right, bottom]: [f32; 4]) -> (f32, f32, f32, f32) {
    (
        left,
        top,
        (size.x - left - right).max(0.0),
        (size.y - top - bottom).max(0.0),
    )
}

/// Make `node` findable by `object`.
pub fn publish<T: Interface>(object: &T, node: &NodeRef) {
    crate::identity::publish(object, node);
}

/// The node behind `object`, if this DLL made it.
pub fn resolve<T: Interface>(object: &T) -> Option<NodeRef> {
    crate::identity::lookup::<_, Mutex<Node>>(object)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(opacity: f32) -> NodeRef {
        let node = Node::new();
        node.lock().unwrap().opacity = opacity;
        node
    }

    #[test]
    fn a_snapshot_carries_the_offset_and_size() {
        let root = node(1.0);
        {
            let mut root = root.lock().unwrap();
            root.offset = Vector3 {
                x: 4.0,
                y: 8.0,
                z: 0.0,
            };
            root.size = Vector2 { x: 16.0, y: 32.0 };
            root.color = [1.0, 0.0, 0.0, 1.0];
        }
        let visual = snapshot(&root).unwrap();
        assert_eq!(visual.offset, [4.0, 8.0]);
        assert_eq!(visual.size, [16.0, 32.0]);
        assert_eq!(visual.color, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn a_relative_root_fills_the_host() {
        let root = node(1.0);
        {
            let mut root = root.lock().unwrap();
            root.relative_size = Some(Vector2 { x: 1.0, y: 0.5 });
            root.relative_offset = Some(Vector3 {
                x: 0.0,
                y: 0.5,
                z: 0.0,
            });
        }
        let visual = snapshot_in(&root, Vector2 { x: 800.0, y: 600.0 }).unwrap();
        assert_eq!(visual.size, [800.0, 300.0]);
        assert_eq!(visual.offset, [0.0, 300.0]);
    }

    #[test]
    fn a_relative_child_resolves_against_its_parent() {
        let parent = node(1.0);
        {
            let mut parent = parent.lock().unwrap();
            parent.size = Vector2 { x: 200.0, y: 100.0 };
        }
        let child = node(1.0);
        child.lock().unwrap().relative_size = Some(Vector2 { x: 0.5, y: 0.5 });
        parent.lock().unwrap().children.push(child);

        let visual = snapshot(&parent).unwrap();
        assert_eq!(visual.children[0].size, [100.0, 50.0]);
    }

    #[test]
    fn child_opacity_is_multiplied_downwards() {
        let parent = node(0.5);
        let child = node(0.5);
        parent.lock().unwrap().children.push(child);
        let visual = snapshot(&parent).unwrap();
        assert_eq!(visual.opacity, 0.5);
        assert_eq!(visual.children[0].opacity, 0.25);
    }

    #[test]
    fn an_invisible_child_is_dropped_with_its_subtree() {
        let parent = node(1.0);
        let child = node(1.0);
        {
            let grandchild = node(1.0);
            child.lock().unwrap().children.push(grandchild);
            child.lock().unwrap().visible = false;
        }
        parent.lock().unwrap().children.push(child);
        let visual = snapshot(&parent).unwrap();
        assert!(visual.children.is_empty());
    }

    #[test]
    fn insets_become_a_clip_inside_the_visual() {
        let root = node(1.0);
        {
            let mut root = root.lock().unwrap();
            root.size = Vector2 { x: 100.0, y: 50.0 };
            root.insets = Some([10.0, 5.0, 20.0, 5.0]);
        }
        let visual = snapshot(&root).unwrap();
        assert_eq!(visual.clip, Some((10.0, 5.0, 70.0, 40.0)));
    }

    #[test]
    fn insets_larger_than_the_visual_clip_everything() {
        let root = node(1.0);
        {
            let mut root = root.lock().unwrap();
            root.size = Vector2 { x: 10.0, y: 10.0 };
            root.insets = Some([20.0, 20.0, 20.0, 20.0]);
        }
        let visual = snapshot(&root).unwrap();
        assert_eq!(visual.clip, Some((20.0, 20.0, 0.0, 0.0)));
    }

    #[test]
    fn an_invisible_root_yields_nothing() {
        let root = node(1.0);
        root.lock().unwrap().visible = false;
        assert!(snapshot(&root).is_none());
    }

    #[test]
    fn an_animation_drives_the_property_it_names() {
        use opencomp_core::Frame;
        let root = node(1.0);
        root.lock().unwrap().opacity = 1.0;
        start_animation(
            &root,
            Property::Opacity,
            Keyframes {
                frames: vec![Frame {
                    time: 0.0,
                    value: [0.25, 0.25, 0.25, 0.25],
                    easing: opencomp_core::Easing::Linear,
                }],
                ..Keyframes::default()
            },
        );
        let visual = snapshot(&root).unwrap();
        assert_eq!(visual.opacity, 0.25);

        stop_animation(&root, Property::Opacity);
        assert_eq!(snapshot(&root).unwrap().opacity, 1.0);
    }

    #[test]
    fn a_centred_anchor_places_the_offset_at_the_middle() {
        let root = node(1.0);
        {
            let mut root = root.lock().unwrap();
            root.size = Vector2 { x: 100.0, y: 40.0 };
            root.offset = Vector3 {
                x: 50.0,
                y: 20.0,
                z: 0.0,
            };
            root.anchor = Vector2 { x: 0.5, y: 0.5 };
        }
        let visual = snapshot(&root).unwrap();
        assert_eq!(visual.offset, [0.0, 0.0]);
        assert_eq!(visual.size, [100.0, 40.0]);
    }

    #[test]
    fn scale_and_rotation_reach_the_snapshot() {
        let root = node(1.0);
        {
            let mut root = root.lock().unwrap();
            root.scale = Vector3 {
                x: 2.0,
                y: 3.0,
                z: 1.0,
            };
            root.rotation = std::f32::consts::FRAC_PI_4;
            root.center = Vector3 {
                x: 4.0,
                y: 5.0,
                z: 0.0,
            };
        }
        let visual = snapshot(&root).unwrap();
        assert_eq!(visual.scale, [2.0, 3.0]);
        assert_eq!(visual.rotation, std::f32::consts::FRAC_PI_4);
        assert_eq!(visual.center, [4.0, 5.0]);
    }

    #[test]
    fn a_cycle_stops_at_the_depth_limit() {
        let parent = node(1.0);
        parent.lock().unwrap().children.push(parent.clone());
        let visual = snapshot(&parent).unwrap();
        assert_eq!(visual.children.len(), 1);
    }
}
