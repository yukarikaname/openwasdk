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

use crate::clock::{Clock, Playback};
use opencomp_core::{Keyframes, Visual as CoreVisual};
use windows_core::Interface;
use windows_numerics::{Vector2, Vector3};

#[cfg(windows)]
use crate::bindings::Microsoft::UI::Composition::CompositionShape;

/// A node in the live tree. Shared between the COM object, its parent and any snapshot in flight.
pub type NodeRef = Arc<Mutex<Node>>;

pub struct Node {
    /// The compositor clock every animation on this node reads.
    pub clock: Arc<Clock>,
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
    /// Corner radii clockwise from the top-left, from a rounded geometric clip.
    pub radius: [f32; 4],
    /// Fraction of the parent's size, from `IVisual2`. A root visual uses this to fill its host.
    pub relative_size: Option<Vector2>,
    pub relative_offset: Option<Vector3>,
    /// The surface a surface brush attached, named by the id it allocates.
    pub surface: Option<u32>,
    /// The gradient a gradient brush attached; resolved against the node's size per frame.
    #[cfg(windows)]
    pub gradient: Option<crate::gradient::Spec>,
    /// The shapes a `ShapeVisual` draws, shared with its `Shapes` collection. A plain visual
    /// has none.
    #[cfg(windows)]
    pub shapes: Option<Arc<Mutex<Vec<CompositionShape>>>>,
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
    /// The clock time at which the animation began.
    pub started: f32,
    /// The controller driving it, if the client started it with one.
    pub playback: Option<Arc<Playback>>,
}

impl Running {
    /// The animation's local time at the clock instant `now`.
    fn local_time(&self, now: f32) -> f32 {
        let mut span = self.keyframes.duration;
        if span <= 0.0 {
            span = self.keyframes.frames.last().map_or(0.0, |frame| frame.time);
        }
        let delay = self.keyframes.delay.max(0.0);
        match &self.playback {
            Some(playback) => playback.local_time(now, span, delay),
            None => now - self.started,
        }
    }
}

impl Default for Node {
    fn default() -> Self {
        Self::detached(Clock::new())
    }
}

impl Node {
    fn detached(clock: Arc<Clock>) -> Self {
        Self {
            clock,
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
            radius: [0.0; 4],
            relative_size: None,
            relative_offset: None,
            surface: None,
            #[cfg(windows)]
            gradient: None,
            #[cfg(windows)]
            shapes: None,
            animations: Vec::new(),
            children: Vec::new(),
        }
    }

    /// A node bound to a compositor clock.
    pub fn with_clock(clock: Arc<Clock>) -> NodeRef {
        Arc::new(Mutex::new(Self::detached(clock)))
    }

    pub fn new() -> NodeRef {
        Arc::new(Mutex::new(Self::default()))
    }

    /// The animated value of each property at `now`, in draw order.
    fn animation_values(&self, now: f32) -> impl Iterator<Item = (Property, [f32; 4])> + '_ {
        self.animations.iter().filter_map(move |running| {
            running
                .keyframes
                .sample(running.local_time(now))
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
    let now = node.lock().unwrap().clock.now();
    snapshot_at(node, parent, 0, now)
}

/// The same, sampled at an explicit compositor time.
pub fn snapshot_in_at(node: &NodeRef, parent: Vector2, now: f32) -> Option<CoreVisual> {
    snapshot_at(node, parent, 0, now)
}

fn snapshot_at(node: &NodeRef, parent: Vector2, depth: u32, now: f32) -> Option<CoreVisual> {
    let (mut visual, size, children, _shapes) = {
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
        // Only this node's own opacity: `opencomp-core` multiplies it down the tree, and
        // storing the product here would apply every ancestor twice.
        let mut opacity = node.opacity.clamp(0.0, 1.0);
        let mut color = node.color;
        for (property, value) in node.animation_values(now) {
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
        // A relative gradient resolves against the size this visual has now, so resizing the
        // sprite moves the ramp with it instead of freezing the fractions taken at SetBrush.
        #[cfg(windows)]
        let gradient = node.gradient.as_ref().and_then(|spec| spec.resolve(size));
        let visual = CoreVisual {
            offset,
            size: [size.x, size.y],
            color,
            opacity,
            scale: [node.scale.x, node.scale.y],
            rotation: node.rotation,
            center: [node.center.x, node.center.y],
            // A visual has no surface until a surface brush names one.
            image: node.surface,
            #[cfg(windows)]
            gradient,
            #[cfg(not(windows))]
            gradient: None,
            uv: [0.0, 0.0, 1.0, 1.0],
            radius: node.radius,
            shape: None,
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
        #[cfg(windows)]
        let shapes = node.shapes.clone();
        #[cfg(not(windows))]
        let shapes = ();
        (visual, size, children, shapes)
    };
    for child in &children {
        if let Some(child) = snapshot_at(child, size, depth + 1, now) {
            visual.children.push(child);
        }
    }
    // A shape visual paints its shapes over its children; each shape is a draw of its own,
    // the same way a sprite is, so the transform and paint live in one node.
    #[cfg(windows)]
    if let Some(shapes) = _shapes {
        let shapes = shapes.lock().unwrap().clone();
        for shape in &shapes {
            if let Some(child) = crate::shape::visual(shape, size) {
                visual.children.push(child);
            }
        }
    }
    Some(visual)
}

/// Start an animation, replacing one already running on the same property.
pub fn start_animation(node: &NodeRef, property: Property, keyframes: Keyframes) {
    start_with(node, property, keyframes, None);
}

/// Start an animation driven by a controller.
pub fn start_animation_with_controller(
    node: &NodeRef,
    property: Property,
    keyframes: Keyframes,
    playback: Arc<Playback>,
) {
    start_with(node, property, keyframes, Some(playback));
}

fn start_with(
    node: &NodeRef,
    property: Property,
    keyframes: Keyframes,
    playback: Option<Arc<Playback>>,
) {
    let mut node = node.lock().unwrap();
    let now = node.clock.now();
    if let Some(playback) = &playback {
        playback.bind(now);
    }
    node.animations
        .retain(|running| running.property != property);
    node.animations.push(Running {
        property,
        keyframes,
        started: now,
        playback,
    });
}

/// The controller behind the animation running on `property`, if it has one.
pub fn playback_for(node: &NodeRef, property: Property) -> Option<Arc<Playback>> {
    node.lock()
        .unwrap()
        .animations
        .iter()
        .find(|running| running.property == property)
        .and_then(|running| running.playback.clone())
}

/// Stop an animation and leave the property at the value it had reached.
///
/// `AnimationStopBehavior.LeaveCurrentValue` is the default the ABI reports, so the
/// last sampled value is written into the field the snapshot reads.
pub fn stop_animation(node: &NodeRef, property: Property) {
    let mut node = node.lock().unwrap();
    let running = node
        .animations
        .iter()
        .position(|running| running.property == property);
    let now = node.clock.now();
    let Some(running) = running.map(|at| node.animations.remove(at)) else {
        return;
    };
    let Some(value) = running.keyframes.sample(running.local_time(now)) else {
        return;
    };
    match property {
        Property::Offset => {
            node.offset = Vector3 {
                x: value[0],
                y: value[1],
                z: node.offset.z,
            }
        }
        Property::Size => {
            node.size = Vector2 {
                x: value[0],
                y: value[1],
            }
        }
        Property::Opacity => node.opacity = value[0].clamp(0.0, 1.0),
        Property::Color => node.color = value,
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

/// Make `node` findable by `object`.
pub fn publish<T: Interface>(object: &T, node: &NodeRef) {
    crate::identity::publish(object, node);
}

/// The node behind `object`, if this DLL made it.
pub fn resolve<T: Interface>(object: &T) -> Option<NodeRef> {
    crate::identity::lookup::<_, Mutex<Node>>(object)
}

#[cfg(test)]
mod tests;
