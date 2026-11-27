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
fn each_node_carries_only_its_own_opacity() {
    // The core multiplies down the tree; a product here would apply ancestors twice.
    let parent = node(0.5);
    let child = node(0.5);
    parent.lock().unwrap().children.push(child);
    let visual = snapshot(&parent).unwrap();
    assert_eq!(visual.opacity, 0.5);
    assert_eq!(visual.children[0].opacity, 0.5);
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

    // LeaveCurrentValue: the property keeps what the animation reached.
    stop_animation(&root, Property::Opacity);
    assert_eq!(snapshot(&root).unwrap().opacity, 0.25);
}

#[test]
fn an_animation_samples_the_compositor_clock() {
    use opencomp_core::Frame;
    let clock = Clock::new();
    let root = Node::with_clock(clock.clone());
    start_animation(
        &root,
        Property::Opacity,
        Keyframes {
            frames: vec![
                Frame {
                    time: 0.0,
                    value: [0.0; 4],
                    easing: opencomp_core::Easing::Linear,
                },
                Frame {
                    time: 1.0,
                    value: [1.0; 4],
                    easing: opencomp_core::Easing::Linear,
                },
            ],
            ..Keyframes::default()
        },
    );

    assert_eq!(snapshot(&root).unwrap().opacity, 0.0);
    clock.tick(0.5);
    assert_eq!(snapshot(&root).unwrap().opacity, 0.5);
    // A caller can ask for a different instant without moving the clock.
    let visual = snapshot_in_at(&root, Vector2::default(), 0.25).unwrap();
    assert_eq!(visual.opacity, 0.25);
    assert_eq!(clock.now(), 0.5);
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
