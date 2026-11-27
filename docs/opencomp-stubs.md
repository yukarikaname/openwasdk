# Composition stubs

`crates/opencomp` answers the `Microsoft.UI.Composition` ABI, but not every method is real.
This page is the survey of every `E_NOTIMPL` body and the reason it is still one.

At survey time there were **129 `E_NOTIMPL` bodies**. After the work recorded here:

| state | count |
| --- | --- |
| `E_NOTIMPL` bodies remaining | 94 |
| stub bodies replaced with real code | 36 |
| methods fully answered | 36 |
| partial methods (real value, honest fallback) | 7 |

The arithmetic is `129 - 36 + 1 = 94`: `animation_group::stop` added one fallback of its own for
an animation group this DLL did not make. A fallback that refuses a foreign object is preferable
to a silent no-op; a caller learns which layer could not answer.

A method marked **later** stays `E_NOTIMPL` because the thing it needs does not exist here. The
note says which thing. A method marked **partial** keeps a real value and only returns
`E_NOTIMPL` for the case the generated `*_Impl` trait cannot express: a null WinRT property or a
foreign object.

## Implemented

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

### Partial methods

These do real work but still contain one `E_NOTIMPL` branch. They also appear in the remaining
table below, because the table lists current code sites. `StartAnimationGroup`, `Geometry` and
`Surface` were already partial before this survey; the other four gained their real half here.

| interface | method | real part | fallback |
| --- | --- | --- | --- |
| `ICompositionObject2` | `StopAnimationGroup` | stops every animation the group started, by target name | a foreign group keeps no list here |
| `IContentIsland` | `AppData` | returns stored data | null data cannot be expressed as a return value |
| `IContentIsland` | `LayoutDirectionOverride` | returns the stored override | null override cannot be expressed |
| `IVisual2` | `ParentForTransform` | returns the stored visual | null visual cannot be expressed |
| `ICompositionGeometricClip` | `Geometry` | returns the stored geometry | null geometry cannot be expressed |
| `ICompositionSurfaceBrush` | `Surface` | returns the stored surface | null surface cannot be expressed |

## Remaining `E_NOTIMPL`

### `ICompositionObject2` (shared by every composition object)

| method | status | note |
| --- | --- | --- |
| `Comment` | later | no per-object comment storage; only `ICompositor5` keeps one |
| `SetComment` | later | same |
| `ImplicitAnimations` | later | needs an `ImplicitAnimationCollection` object and trigger state |
| `SetImplicitAnimations` | later | same |
| `StartAnimationGroup` | partial | starts groups this DLL made; a foreign group has no list |
| `StopAnimationGroup` | partial | stops groups this DLL made; a foreign group has no list |
| `TryGetAnimationController` | later | needs the `AnimationController` object from `ICompositor8` |
| `StartAnimationWithController` | later | same controller object |

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

### `ICompositor`

| method | status | note |
| --- | --- | --- |
| `CreateEffectFactory` | later | needs `Windows.Graphics.Effects` and a Direct2D effect pipeline |
| `CreateEffectFactoryWithProperties` | later | same |
| `CreateExpressionAnimation` | later | needs an expression parser and evaluator |
| `CreateExpressionAnimationWithExpression` | later | same |
| `CreateQuaternionKeyFrameAnimation` | later | no quaternion animation object; `Node` has no quaternion channel |
| `CreateScopedBatch` | later | needs a batch object and commit tracking; there is no renderer commit |
| `CreateVector4KeyFrameAnimation` | later | no vector4 animation object; `Node` has no vector4 channel |
| `GetCommitBatch` | later | same batch and commit state |

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
| `CreateColorGradientStop` | later | gradient stop object type |
| `CreateColorGradientStopWithOffsetAndColor` | later | same |
| `CreateLinearGradientBrush` | later | gradient brush object and renderer gradient pipeline |
| `CreateSpringScalarAnimation` | later | spring natural-motion animation object and solver |
| `CreateSpringVector2Animation` | later | same |
| `CreateSpringVector3Animation` | later | same |

### `ICompositor5`

| method | status | note |
| --- | --- | --- |
| `CreateBounceScalarAnimation` | later | bounce natural-motion animation object and solver |
| `CreateBounceVector2Animation` | later | same |
| `CreateBounceVector3Animation` | later | same |
| `CreateContainerShape` | later | shape/geometry object type |
| `CreateEllipseGeometry` | later | geometry object type the renderer cannot cut |
| `CreateLineGeometry` | later | same |
| `CreatePathGeometry` | later | path geometry object and path flattening |
| `CreatePathGeometryWithPath` | later | same |
| `CreatePathKeyFrameAnimation` | later | path animation object and geometry morphing |
| `CreateRectangleGeometry` | later | geometry object type the renderer cannot cut |
| `CreateShapeVisual` | later | shape visual object and renderer fill/stroke |
| `CreateSpriteShape` | later | sprite shape object and renderer fill/stroke |
| `CreateSpriteShapeWithGeometry` | later | same |
| `CreateViewBox` | later | `CompositionViewBox` object type |
| `RequestCommitAsync` | later | needs a commit scheduler and `IAsyncAction` completion |

### `ICompositor6`

| method | status | note |
| --- | --- | --- |
| `CreateRedirectVisual` | later | redirect visual object and renderer redirection |
| `CreateRedirectVisualWithSourceVisual` | later | same |
| `CreateBooleanKeyFrameAnimation` | later | boolean animation object; `Node` has no boolean channel |

### `ICompositor8` and shadow/gradient/surface extensions

| interface | method | status | note |
| --- | --- | --- | --- |
| `ICompositor8` | `CreateAnimationController` | later | controller object and per-property animation handles |
| `ICompositorWithProjectedShadow` | `CreateProjectedShadowCaster` | later | projected-shadow object types and renderer pass |
| `ICompositorWithProjectedShadow` | `CreateProjectedShadow` | later | same |
| `ICompositorWithProjectedShadow` | `CreateProjectedShadowReceiver` | later | same |
| `ICompositorWithRadialGradient` | `CreateRadialGradientBrush` | later | radial gradient brush and renderer pipeline |
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

### `ICompositionGraphicsDeviceInterop`

| method | status | note |
| --- | --- | --- |
| `GetRenderingDevice` | later | creation always stores a device; the `None` branch is a defensive path |

## What deliberately stays `E_NOTIMPL`

The renderer is a tree of rectangles, textures and rounded corners. Anything that would need a
second renderer path is left out rather than accepted and dropped:

- **Lights, shadows, effects, shapes and gradients** have no draw path in `opencomp-core`, and a
  getter that returned a configured object would promise a picture that never appears.
- **Expressions and implicit animations** need an evaluator and a trigger, not just a field.
- **Controllers, scoped batches and commits** need an animation/commit clock; this DLL samples
  time when a frame asks for a tree.
- **Window z-order and dispatcher queues** belong to the host window manager and message loop.
- **Null WinRT properties** cannot be returned through the generated `*_Impl` traits. The setter
  is real; the unset read reports `E_NOTIMPL` instead of inventing an object.
- **Scrolling** would have to move the GPU texture; moving only the CPU buffer would make the
  snapshot and the drawn surface disagree.
