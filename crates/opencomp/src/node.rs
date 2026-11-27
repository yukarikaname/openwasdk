//! The state behind every WinRT visual, and the snapshot the renderer consumes.
//!
//! A `Visual` in the composition API is a COM object with children that can be edited in any
//! order. The renderer wants a tree it can walk. Both views share one node: the COM object keeps
//! the node alive, the parent keeps its children as nodes, and each frame walks the nodes.
//!
//! Nodes are also published here by identity, because `VisualCollection::InsertAtTop` receives a
//! COM pointer and has to find the node behind it. The table holds weak references, so a visual
//! that dies leaves nothing behind.

use std::sync::{Arc, Mutex, OnceLock, Weak};

use opencomp_core::Visual as CoreVisual;
use windows_core::{IUnknown, Interface};
use windows_numerics::{Vector2, Vector3};

/// A node in the live tree. Shared between the COM object, its parent and any snapshot in flight.
pub type NodeRef = Arc<Mutex<Node>>;

pub struct Node {
    pub offset: Vector3,
    pub size: Vector2,
    pub opacity: f32,
    pub visible: bool,
    pub color: [f32; 4],
    /// Left, top, right, bottom insets of an `InsetClip`, if one is attached.
    pub insets: Option<[f32; 4]>,
    pub children: Vec<NodeRef>,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            offset: Vector3::default(),
            size: Vector2::default(),
            opacity: 1.0,
            visible: true,
            color: [0.0; 4],
            insets: None,
            children: Vec::new(),
        }
    }
}

impl Node {
    pub fn new() -> NodeRef {
        Arc::new(Mutex::new(Self::default()))
    }
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

/// Copy the tree into the renderer's form. A node that is not visible yields nothing, so its
/// subtree disappears with it.
pub fn snapshot(node: &NodeRef) -> Option<CoreVisual> {
    snapshot_at(node, 1.0, 0)
}

fn snapshot_at(node: &NodeRef, parent_opacity: f32, depth: u32) -> Option<CoreVisual> {
    let (mut visual, children) = {
        let node = node.lock().unwrap();
        if !node.visible {
            return None;
        }
        let visual = CoreVisual {
            offset: [node.offset.x, node.offset.y],
            size: [node.size.x, node.size.y],
            color: node.color,
            opacity: (parent_opacity * node.opacity).clamp(0.0, 1.0),
            clip: node.insets.map(|insets| clip_of(node.size, insets)),
            children: Vec::new(),
        };
        // A visual inserted into its own subtree would recurse until the stack runs out; the
        // bound keeps such a tree renderable instead of fatal.
        let children = if depth < 64 {
            node.children.clone()
        } else {
            Vec::new()
        };
        (visual, children)
    };
    for child in &children {
        if let Some(child) = snapshot_at(child, visual.opacity, depth + 1) {
            visual.children.push(child);
        }
    }
    Some(visual)
}

/// A weak node filed under the identity of the object that owns it.
type Entry = (usize, Weak<Mutex<Node>>);

/// Nodes by object identity. Keyed by the `IUnknown` pointer, which is the one pointer every
/// interface of an object agrees on.
static REGISTRY: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();

fn registry() -> &'static Mutex<Vec<Entry>> {
    REGISTRY.get_or_init(|| Mutex::new(Vec::new()))
}

/// The identity of a COM object, normalised through `IUnknown`.
fn key<T: Interface>(object: &T) -> Option<usize> {
    object
        .cast::<IUnknown>()
        .ok()
        .map(|unknown| unknown.as_raw() as usize)
}

/// Make `node` findable by `object`.
pub fn publish<T: Interface>(object: &T, node: &NodeRef) {
    let Some(key) = key(object) else { return };
    let mut table = registry().lock().unwrap();
    if table.len() >= 64 {
        table.retain(|(_, node)| node.strong_count() > 0);
    }
    table.retain(|(existing, _)| *existing != key);
    table.push((key, Arc::downgrade(node)));
}

/// The node behind `object`, if this DLL made it.
pub fn resolve<T: Interface>(object: &T) -> Option<NodeRef> {
    let key = key(object)?;
    let table = registry().lock().unwrap();
    table
        .iter()
        .find(|(existing, _)| *existing == key)
        .and_then(|(_, node)| node.upgrade())
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
    fn a_cycle_stops_at_the_depth_limit() {
        let parent = node(1.0);
        parent.lock().unwrap().children.push(parent.clone());
        let visual = snapshot(&parent).unwrap();
        assert_eq!(visual.children.len(), 1);
    }
}
