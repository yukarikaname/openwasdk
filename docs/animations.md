# Animations

An animation is an object a client builds and attaches to a visual. It becomes a value on a node,
which the snapshot copies into the tree, which the renderer draws. There is no animation thread and
no clock: the value is sampled when a frame asks for the tree.

## The path from a call to a pixel

1. The client asks the compositor for an animation, inserts key frames into it and sets its timing.
2. `StartAnimation("Opacity", animation)` on a visual looks up the curve behind the animation
   object, maps the property name to a node field, and records the instant.
3. Each snapshot samples every running curve against `Instant::now()` and writes the result into
   the node's field.
4. The snapshot becomes a `Visual` tree and the renderer draws it (see [`rendering.md`](rendering.md)).

## The objects a client builds

| factory | `InsertKeyFrame` value | channels |
| --- | --- | --- |
| `CreateScalarKeyFrameAnimation` | `float` | the value copied into all four |
| `CreateVector2KeyFrameAnimation` | `Vector2` | `x, y, 0, 0` |
| `CreateVector3KeyFrameAnimation` | `Vector3` | `x, y, z, 0` |
| `CreateColorKeyFrameAnimation` | `Windows.UI.Color` | linear RGBA, decoded as a brush is |

Both `InsertKeyFrame(time, value)` and `InsertKeyFrameWithEasingFunction(time, value, easing)` are
implemented. Frames are kept sorted by time; a repeated time is inserted after the equal ones.
`ColorKeyFrameAnimation.InterpolationColorSpace` is accepted and ignored: colour channels are
already linear, and interpolation is linear in linear light.

Timing comes from `IKeyFrameAnimation`, in `TimeSpan` units of 100 ns:

| property | meaning |
| --- | --- |
| `Duration` | seconds; `0` means "as long as the last key frame" |
| `DelayTime` | seconds held at the first frame before sampling starts; defaults to 0 |
| `IterationCount` | repeats; `0` means forever, as does `IterationBehavior::Forever` |
| `KeyFrameCount` | number of frames inserted |
| `StopBehavior` | accepted; only `LeaveCurrentValue` is answered (see below) |

Easing functions are `LinearEasingFunction` and `CubicBezierEasingFunction`, whose two control
points are kept. A cubic is solved with sixteen bisections on `x` and then read as `y`, enough for
a 60 Hz frame. The curve stored on a frame bends the segment that leaves it.

Expression key frames are `E_NOTIMPL`, and `ICompositionAnimation`'s `SetScalarParameter` family
is accepted and dropped — they exist for expression animations.

## What a sample means

`Keyframes::sample` in `opencomp-core` is the whole contract, and it is testable without a window:

- before the first frame, and after the last, the value is held;
- `DelayTime` holds the first value until it elapses;
- the local time wraps modulo the duration when iterations remain, and stops at
  `duration * iterations` when they run out;
- each channel interpolates on its own, linearly, with the leaving frame's easing applied to the
  progress between frames;
- an empty animation has no value and changes nothing.

## Where the state lives

The curve is published in the identity table (`crates/opencomp/src/identity.rs`), keyed by the
animation object's `IUnknown` pointer, which every interface of the object agrees on. The table
holds weak references and type-erased values, so the same table maps a visual to its
`Arc<Mutex<Node>>` and an animation to its `Arc<Mutex<Keyframes>>` without either leaking.

`StartAnimation` therefore needs an animation this DLL made. One from another server has no entry:
the call returns `S_OK` and nothing animates. The node keeps a cloned `Keyframes`, so releasing
the animation object on the client side does not stop it.

## `StartAnimation`

`ICompositionObject.StartAnimation` parses the property name exactly, case included:

| name | field it drives |
| --- | --- |
| `Offset` | `offset.x`, `offset.y`; `z` keeps the node's value |
| `Size` | `size.x`, `size.y` |
| `Opacity` | channel 0, clamped to 0..=1 |
| `Color` | the whole `color`, linear |

Any other name is ignored and returns `S_OK`. Starting a second animation on the same property
drops the first one for that property only. The start instant is recorded when the call arrives.

The call exists on every object, but only visuals own a node: on a brush, clip, easing function or
property set it accepts the animation and does nothing. Animating a brush's `Color` would not reach
the tree in any case: `SetBrush` copies the colour into the visual when the brush is attached, and
nothing links the two afterwards.

## The snapshot

`node.rs` walks the tree once per snapshot. For each node it resolves relative sizes and offsets,
multiplies the parent's opacity in, then lets every running animation overwrite its field, in the
order the animations were started. Children then inherit the animated `opacity` as their parent
opacity. Two consequences worth knowing:

- an opacity animation **replaces** the product of the ancestors' opacities rather than being one
  more factor, so animating a child's opacity also discards what its parent had;
- a `Size` animation overrides a relative size, because it is applied after `relative_size` is
  resolved.

Sampling happens per snapshot, not on a tick: the elapsed time is read inside the copy, so the
value moves once per frame the host asks for and never in between. Nothing runs while no snapshot
is taken, and no interpolation happens across snapshots.

## `StopAnimation`

`StopAnimation("Opacity")` removes the running entry and writes the value the curve had reached
into the node, which is what `AnimationStopBehavior.LeaveCurrentValue` — the default the object
reports — means: the visual stays where the animation stopped. Unknown names and properties with
nothing running are ignored.

## A worked example

```cpp
using namespace winrt::Microsoft::UI::Composition;

Compositor compositor;
auto sprite = compositor.CreateSpriteVisual();
sprite.Size({200.0f, 100.0f});

auto animation = compositor.CreateScalarKeyFrameAnimation();
animation.Duration(std::chrono::seconds(1));
animation.InsertKeyFrame(0.0f, 0.0f);
animation.InsertKeyFrame(1.0f, 1.0f,
    compositor.CreateCubicBezierEasingFunction({0.42f, 0.0f}, {0.58f, 1.0f}));
sprite.StartAnimation(L"Opacity", animation);
```

One iteration, one second, no delay. Sampling the tree at three instants after `StartAnimation`:

| time since start | local time | progress | `Opacity` |
| --- | --- | --- | --- |
| 0 ms | 0 s | 0.0 | 0.0 |
| 250 ms | 0.25 s | 0.25 | 0.129 |
| 2000 ms | clamped to 1 s | 1.0 | 1.0 |

`0.129` is the easing curve, not the time: the cubic maps `x = 0.25` to `y ≈ 0.129`. After two
seconds the single iteration is spent, the local time is clamped to the end and the last value is
held for as long as the animation is attached.

## What is not there

| feature | state |
| --- | --- |
| expression animations | `CreateExpressionAnimation` and `InsertExpressionKeyFrame` are `E_NOTIMPL`, parameters are dropped |
| animation groups | implemented for groups this DLL made, through `ICompositionAnimation2.Target`; `StopAnimationGroup` is `E_NOTIMPL` |
| implicit animations | `CreateImplicitAnimationCollection` and `ICompositionObject2.ImplicitAnimations` are `E_NOTIMPL` |
| `Target` | `ICompositionAnimation2` is not implemented, so its `QueryInterface` fails; the property name is the whole binding |
| controllers | `TryGetAnimationController`, `CreateAnimationController`, `StartAnimationWithController` are `E_NOTIMPL`: no pause, seek or playback rate |
| natural motion | spring, bounce, quaternion, vector4, boolean and path animations are `E_NOTIMPL` |
| completion | scoped batches are `E_NOTIMPL`; nothing signals that an animation ended |
| clock | no `CompositionTarget` is served and nothing ticks; frames are whatever the host presents |

Two smaller mismatches: `IterationCount` reads back `1` after `Forever`, because the getter answers
`max(1)` for a stored `0`, and `SetIterationBehavior(Count)` leaves the stored count alone.
`GlobalPlaybackRate` answers `1.0` and its setter does nothing.
