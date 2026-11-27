# Composition stubs

`crates/opencomp` answers the `Microsoft.UI.Composition` ABI, but not every method is real.
This page is the survey of every `E_NOTIMPL` body and the reason it is still one.

At survey time there were **129 `E_NOTIMPL` bodies**. After the work recorded here:

| state | count |
| --- | --- |
| `E_NOTIMPL` bodies remaining | 93 |
| stub bodies replaced with real code | 45 |
| methods fully answered | 45 |

The number above is asserted by `crates/opencomp/tests/stubs.rs`, which counts the calls in
the crate: a page that disagrees with the code is worse than no page. A method that does its
work and still refuses one case (a foreign animation group, a gradient property the draw list
does not model) counts as a stub site here, which is why the arithmetic below the line is not
`129 - 45`.

The four bodies removed in this pass are `GetCommitBatch`, `CreateScopedBatch`,
`RequestCommitAsync` and `CreateAnimationController`. Each one is backed by the compositor clock
described below. The shape pass then replaced the five shape factories and gained three new
stub sites: the two view-box methods and the dash-array getter, all explained in their tables.

A method marked **later** stays `E_NOTIMPL` because the thing it needs does not exist here. The
note says which thing. A method marked **partial** keeps a real value and only returns
`E_NOTIMPL` for the case the generated `*_Impl` trait cannot express: a null WinRT property or a
foreign object.

## Implemented

### The compositor clock and commit completion

Each `Compositor` owns a clock. A visual created by that compositor carries the clock in its
node, and every animation start and every snapshot samples that time rather than reading the
system clock. The frame loop in `present.rs` is the only caller that advances it: it draws the
tree at the current time, then calls `tick`, which closes open commit batches and runs the
completions they queued. A bridge created from a compositor drives that compositor's clock, so
`GetCommitBatch`/`Completed` is tied to the frames the client actually sees.

| interface | method | what it now does |
| --- | --- | --- |
| `ICompositor` | `GetCommitBatch` | returns a batch that is active until the next frame |
| `ICompositor` | `CreateScopedBatch` | returns a batch that stays active until the client ends it |
| `ICompositor5` | `RequestCommitAsync` | completes after the next frame tick, which is the commit boundary |
| `ICompositionCommitBatch` | `IsActive` / `IsEnded` | reports the open/closed state of that batch |
| `ICompositionCommitBatch` | `Completed` / `RemoveCompleted` | queues one completion on the compositor clock when the batch closes |
| `ICompositionScopedBatch` | `End`, `Suspend`, `Resume` | closes once, and suspends/resumes the active state |
| `ICompositionScopedBatch` | `Completed` / `RemoveCompleted` | same queued completion, once per batch |
| `IClosable` on either batch | `Close` | same close as `End`; the metadata has no `Close` on the commit interface itself |
| `ICompositor8` | `CreateAnimationController` | creates a controller over a persistent playback |
| `IAnimationController` | `PlaybackRate`, `Progress`, `ProgressBehavior`, `Pause`, `Resume` | drives the playback a visual samples |
| `ICompositionObject4` | `TryGetAnimationController` | returns the controller of a running animation, if it was started with one |
| `ICompositionObject5` | `StartAnimationWithController` | starts an animation on a visual through that controller |

A batch that is still open at frame time is not an error: the commit boundary closes it and
posts its completion. `ICompositorController` stays out because the metadata gives it no
activation factory, so a client cannot make one to call `CommitNeeded` on; see the last section.

### `ICompositionClip2`, both clip classes

The same 14 property pairs are real on `InsetClip` and `CompositionGeometricClip`. Each keeps its
own `ClipTransform`; `RotationAngle` and `RotationAngleInDegrees` are one field, and an unset
transform reads as the identity matrix, scale `(1, 1)`, and zero vectors.

| method | what it now does |
| --- | --- |
| `AnchorPoint` / `SetAnchorPoint` | stores and returns the anchor |
| `CenterPoint` / `SetCenterPoint` | stores and returns the centre |
| `Offset` / `SetOffset` | stores and returns the offset |
| `RotationAngle` / `SetRotationAngle` | stores radians |
| `RotationAngleInDegrees` / `SetRotationAngleInDegrees` | converts through the same radian field |
| `Scale` / `SetScale` | stores and returns the scale |
| `TransformMatrix` / `SetTransformMatrix` | stores and returns a `Matrix3x2` |

### `Microsoft.UI.Content`

| interface | method | what it now does |
| --- | --- | --- |
| `IContentIsland` | `SetAppData` | stores the client's `IInspectable`; the getter returns it while set |
| `IContentIsland` | `LayoutDirection` | returns the island's direction, left-to-right by default |
| `IContentIsland` | `RasterizationScale` | returns the island's scale, `1.0` by default |
| `IContentIsland` | `AutomationProviderRequested` / `RemoveAutomationProviderRequested` | records a token and the handler; remove forgets both |
| `IContentIsland` | `StateChanged` / `RemoveStateChanged` | records a token and the handler; remove forgets both |
| `IContentSiteBridge` | `SetLayoutDirectionOverride` | unwraps the `IReference` and stores the direction |

### Gradient brushes

| interface | method | what it now does |
| --- | --- | --- |
| `ICompositor4` | `CreateColorGradientStop` | a stop at offset zero with a transparent colour |
| `ICompositor4` | `CreateColorGradientStopWithOffsetAndColor` | a stop the client positions and colours |
| `ICompositor4` | `CreateLinearGradientBrush` | a brush with the documented `(0,0)` to `(1,1)` default ramp |
| `ICompositorWithRadialGradient` | `CreateRadialGradientBrush` | a brush centred on `(0.5, 0.5)` with radius `(0.5, 0.5)` |
| `ICompositionGradientBrush` | `ColorStops` | a live `IVector<ICompositionColorGradientStop>` shared with the brush |
| `ICompositionGradientBrush2` | `MappingMode` / `SetMappingMode` | `Relative` scales by the sprite size, `Absolute` uses node units |
| `ICompositionLinearGradientBrush` | `StartPoint` / `EndPoint` | stored and applied to the ramp |
| `ICompositionRadialGradientBrush` | `EllipseCenter` / `EllipseRadius` | stored and applied to the ramp |
| `ICompositionRadialGradientBrush` | `GradientOriginOffset` | reports the default `(0,0)`; a non-zero origin is refused |
| `ICompositionColorGradientStop` | `Offset` / `Color` | stored and read back through the ABI |

The draw list carries the stops and the shape, so both backends paint the same ramp. Multi-stop
ordering is honoured by sorting on `Offset` when the ramp is built, which is also what
`CreateGradientStopCollection` expects. The gradient transform properties (`AnchorPoint`,
`CenterPoint`, `Offset`, `RotationAngle`, `RotationAngleInDegrees`, `Scale`, `TransformMatrix`)
and the still-unsupported `ExtendMode`/`InterpolationSpace` choices answer through
`gradient::unsupported`; see the fallback table below. The reason is one layer, not seven
spellings: the draw list has no per-gradient affine and no extend or HSL interpolation mode.

### Shapes and geometry

The draw list carries a primitive of its own now: an ellipse (centre and radii) or a line (two
points and a stroke width), in the node's own units. A rounded rectangle is still the rectangle
path with a radius. D2D fills the ellipse through an ellipse geometry and strokes the line with
`DrawLine`; the wgpu preview measures both outlines in the fragment stage. The shape or the
visual's own clip bounds it, and the inherited scissor still applies.

| interface | method | what it now does |
| --- | --- | --- |
| `ICompositor5` | `CreateEllipseGeometry` | an ellipse geometry whose centre and radius round-trip |
| `ICompositor5` | `CreateLineGeometry` | a line geometry whose endpoints round-trip |
| `ICompositor5` | `CreateShapeVisual` | a `ContainerVisual` with a live `Shapes` list |
| `ICompositor5` | `CreateSpriteShape` | a sprite shape with no geometry yet |
| `ICompositor5` | `CreateSpriteShapeWithGeometry` | the same, with the geometry already set |
| `IShapeVisual` | `Shapes` | the live `IVector<CompositionShape>` the snapshot reads |
| `ICompositionShape` | offset, scale, rotation and centre | applied to the shape's drawn node; `TransformMatrix` is stored and answered but not applied, like a visual's |
| `ICompositionSpriteShape` | `FillBrush` / `StrokeBrush` / `StrokeThickness` | the paint the shape's primitive draws with |

An ellipse fills with `FillBrush`; a line strokes with `StrokeBrush` at `StrokeThickness`. The
stroke of an ellipse is not drawn, because a draw carries one paint, not a fill and a stroke at
once. The remaining dash and cap properties are stored and answered, but only the thickness
reaches a pixel. A shape that is not attached to a `ShapeVisual` still answers every property.

Only the ellipse and the line have a draw-list primitive. A sprite shape built on a rounded
rectangle, a path or a container shape paints nothing today; the rounded rectangle still works as
a geometric clip, where it becomes the visual's corner radius.

### Partial methods

These do real work but still contain one `E_NOTIMPL` branch. They also appear in the remaining
table below, because the table lists current code sites. `StartAnimationGroup`, `Geometry` and
`Surface` were already partial before this survey; the others gained their real half here.

| interface | method | real part | fallback |
| --- | --- | --- | --- |
| `ICompositionObject2` | `StopAnimationGroup` | stops every animation the group started, by target name | a foreign group keeps no list here |
| `ICompositionObject4` | `TryGetAnimationController` | returns the controller of an animation this DLL started with one | no controller is null in the ABI |
| `ICompositionObject5` | `StartAnimationWithController` | starts on a visual with a controller this DLL made | a foreign controller keeps no playback here |
| `IContentIsland` | `AppData` | returns stored data | null data cannot be expressed as a return value |
| `IContentIsland` | `LayoutDirectionOverride` | returns the stored override | null override cannot be expressed |
| `IVisual2` | `ParentForTransform` | returns the stored visual | null visual cannot be expressed |
| `ICompositionGeometricClip` | `Geometry` | returns the stored geometry | null geometry cannot be expressed |
| `ICompositionSpriteShape` | `Geometry` | returns the stored geometry | null geometry cannot be expressed |
| `ICompositionSurfaceBrush` | `Surface` | returns the stored surface | null surface cannot be expressed |

## Remaining `E_NOTIMPL`

### `ICompositionObject2` / `4` / `5` (shared by every composition object)

| method | status | note |
| --- | --- | --- |
| `Comment` | later | no per-object comment storage; only `ICompositor5` keeps one |
| `SetComment` | later | same |
| `ImplicitAnimations` | later | needs an `ImplicitAnimationCollection` object and trigger state |
| `SetImplicitAnimations` | later | same |
| `StartAnimationGroup` | partial | starts groups this DLL made; a foreign group has no list |
| `StopAnimationGroup` | partial | stops groups this DLL made; a foreign group has no list |
| `TryGetAnimationController` | partial | returns the controller of an animation started with one; no controller means null in the ABI |
| `StartAnimationWithController` | partial | starts on a visual with a controller this DLL made; a foreign controller is refused |

### `IVisual` / `IVisual2` (shared by both visual classes)

| method | status | note |
| --- | --- | --- |
| `Parent` | later | the tree stores children only; it has no parent link or weak back-reference |
| `ParentForTransform` | partial | stores the value; an unset reference is null in the ABI |

### `ICompositionGeometricClip`

| method | status | note |
| --- | --- | --- |
| `Geometry` | partial | returns the stored geometry; an unset one is null in the ABI |
| `ViewBox` | later | the `CompositionViewBox` object type is not implemented |
| `SetViewBox` | later | same |

### `IShapeVisual`

| method | status | note |
| --- | --- | --- |
| `ViewBox` | later | the `CompositionViewBox` object type is not implemented |
| `SetViewBox` | later | same |

### `ICompositionSpriteShape`

| method | status | note |
| --- | --- | --- |
| `Geometry` | partial | returns the stored geometry; an unset one is null in the ABI |
| `StrokeDashArray` | later | the draw list has no dash pattern for a stroked line |

### `ICompositor`

| method | status | note |
| --- | --- | --- |
| `CreateEffectFactory` | later | needs `Windows.Graphics.Effects` and a Direct2D effect pipeline |
| `CreateEffectFactoryWithProperties` | later | same |
| `CreateExpressionAnimation` | later | needs an expression parser and evaluator |
| `CreateExpressionAnimationWithExpression` | later | same |
| `CreateQuaternionKeyFrameAnimation` | later | no quaternion animation object; `Node` has no quaternion channel |
| `CreateVector4KeyFrameAnimation` | later | no vector4 animation object; `Node` has no vector4 channel |

### `ICompositor2`

| method | status | note |
| --- | --- | --- |
| `CreateAmbientLight` | later | light object type and renderer lighting |
| `CreateBackdropBrush` | later | needs the renderer's backdrop capture path |
| `CreateDistantLight` | later | light object type and renderer lighting |
| `CreateDropShadow` | later | shadow object type and a renderer shadow pass |
| `CreateImplicitAnimationCollection` | later | collection object plus implicit-animation triggers |
| `CreateLayerVisual` | later | layer visual object and an offscreen render target |
| `CreateMaskBrush` | later | mask brush object and renderer masking |
| `CreateNineGridBrush` | later | nine-grid brush object and sampling |
| `CreatePointLight` | later | light object type and renderer lighting |
| `CreateSpotLight` | later | light object type and renderer lighting |

### `ICompositor4`

| method | status | note |
| --- | --- | --- |
| `CreateSpringScalarAnimation` | later | spring natural-motion animation object and solver |
| `CreateSpringVector2Animation` | later | same |
| `CreateSpringVector3Animation` | later | same |

### `ICompositor5`

| method | status | note |
| --- | --- | --- |
| `CreateBounceScalarAnimation` | later | bounce natural-motion animation object and solver |
| `CreateBounceVector2Animation` | later | same |
| `CreateBounceVector3Animation` | later | same |
| `CreateContainerShape` | later | container-shape object and a nested shape list |
| `CreatePathGeometry` | later | path geometry object and path flattening |
| `CreatePathGeometryWithPath` | later | same |
| `CreatePathKeyFrameAnimation` | later | path animation object and geometry morphing |
| `CreateRectangleGeometry` | later | the rounded-rectangle path has a radius but no per-shape offset/size; a rectangle shape would need the shape draw to carry that box |
| `CreateViewBox` | later | `CompositionViewBox` object type |

### `ICompositor6`

| method | status | note |
| --- | --- | --- |
| `CreateRedirectVisual` | later | redirect visual object and renderer redirection |
| `CreateRedirectVisualWithSourceVisual` | later | same |
| `CreateBooleanKeyFrameAnimation` | later | boolean animation object; `Node` has no boolean channel |

### `ICompositor8` and shadow/gradient/surface extensions

| interface | method | status | note |
| --- | --- | --- | --- |
| `ICompositorWithProjectedShadow` | `CreateProjectedShadowCaster` | later | projected-shadow object types and renderer pass |
| `ICompositorWithProjectedShadow` | `CreateProjectedShadow` | later | same |
| `ICompositorWithProjectedShadow` | `CreateProjectedShadowReceiver` | later | same |
| `ICompositorWithVisualSurface` | `CreateVisualSurface` | later | visual surface object and offscreen render |

### `ICompositionSurfaceBrush`

| method | status | note |
| --- | --- | --- |
| `Surface` | partial | returns the stored surface; an unset one is null in the ABI |

### `ICompositionDrawingSurface2`

| method | status | note |
| --- | --- | --- |
| `Scroll` | later | scrolling must move the GPU texture too; only the CPU buffer exists here |
| `ScrollRect` | later | same |
| `ScrollWithClip` | later | same |
| `ScrollRectWithClip` | later | same |

### `ICompositionDrawingSurfaceInterop`

| method | status | note |
| --- | --- | --- |
| `Scroll` | later | the same texture problem as the WinRT surface |
| `ResumeDraw` | later | needs a suspended Direct2D context |
| `SuspendDraw` | later | same |

### `ISpriteVisual2`

| method | status | note |
| --- | --- | --- |
| `Shadow` | later | needs the `CompositionShadow` object type; `CreateDropShadow` is `E_NOTIMPL` |
| `SetShadow` | later | same |

### `IContentSiteBridge`

| method | status | note |
| --- | --- | --- |
| `DispatcherQueue` | later | no dispatcher queue object and no message loop in this DLL |
| `LayoutDirectionOverride` | partial | returns the stored override; an unset one is null in the ABI |

### `IDesktopSiteBridge`

| method | status | note |
| --- | --- | --- |
| `MoveInZOrderAtBottom` | later | z-order belongs to the real window manager |
| `MoveInZOrderAtTop` | later | same |
| `MoveInZOrderBelow` | later | same |

### `IDesktopChildSiteBridge`

| method | status | note |
| --- | --- | --- |
| `SiteView` | later | the `ContentSiteView` object type is not implemented |

### `IContentIsland`

| method | status | note |
| --- | --- | --- |
| `AppData` | partial | returns stored data; null data is not expressible |
| `CoordinateConverter` | later | `ContentCoordinateConverter` object type |
| `CustomProperties` | later | `Windows.Foundation.Collections.IPropertySet` is not implemented |
| `DispatcherQueue` | later | no dispatcher queue object |
| `Environment` | later | `ContentIslandEnvironment` object type |
| `GetAutomationHostProvider` | later | needs an automation provider from the XAML core |
| `GetStateChangeDeferral` | later | `ContentDeferral` object type and state machine |
| `RequestSize` | later | the site owns the size; there is no site resize-request path |

### `IContentIslandStatics`

| method | status | note |
| --- | --- | --- |
| `FindAllForCompositor` | later | islands do not record their compositor |
| `FindAllForCurrentThread` | later | no thread or apartment association |
| `GetByVisual` | later | no reverse visual-to-island map that keeps the island COM object |
| `GetFromId` | later | same |

### Animation objects

| interface | method | status | note |
| --- | --- | --- | --- |
| `IKeyFrameAnimation` | `InsertExpressionKeyFrame` | later | needs an expression parser and evaluator |
| `IKeyFrameAnimation` | `InsertExpressionKeyFrameWithEasingFunction` | later | same |

### Internal fallbacks

These are not ABI methods; they are the helper functions the animation group calls.

| helper | status | note |
| --- | --- | --- |
| `animation_group::start` | later | a foreign group keeps no list this DLL can read |
| `animation_group::stop` | later | same |
| `gradient::unsupported` | later | no per-gradient affine, extend mode or HSL interpolation in the draw list |

### `ICompositionGraphicsDeviceInterop`

| method | status | note |
| --- | --- | --- |
| `GetRenderingDevice` | later | creation always stores a device; the `None` branch is a defensive path |

## What deliberately stays `E_NOTIMPL`

The renderer is a tree of rectangles, textures and rounded corners. Anything that would need a
second renderer path is left out rather than accepted and dropped:

- **Lights, shadows and effects** have no draw path in `opencomp-core`, and a getter that
  returned a configured object would promise a picture that never appears. Gradients and the
  ellipse and line shapes do have one now; paths, container shapes and rectangle shapes still
  do not.
- **Expressions** need a parser and an evaluator, not just a field.
- **`ICompositorController`** needs a controller object, and this metadata gives the class no
  activation factory, so a client cannot make one to call `CommitNeeded` on. `GetCommitBatch`,
  `CreateScopedBatch` and `RequestCommitAsync` already signal the same commit boundary.
- **`ICompositionTarget`** exists in `Windows.UI.Composition`, not `Microsoft.UI.Composition`.
  Claiming the Windows class would hijack another DLL's activation; the root this DLL can
  really present is the one `IDesktopChildSiteBridge.Connect` takes.
- **Implicit animations** need an `ImplicitAnimationCollection` that implements `IMap` and
  `IIterable`, a trigger at every setter, and a way to express "animate from the current
  value" -- the last is not in this key-frame model, which has no null key frame.
- **Window z-order and dispatcher queues** belong to the host window manager and message loop.
- **Null WinRT properties** cannot be returned through the generated `*_Impl` traits. The setter
  is real; the unset read reports `E_NOTIMPL` instead of inventing an object.
- **Scrolling** would have to move the GPU texture; moving only the CPU buffer would make the
  snapshot and the drawn surface disagree.
