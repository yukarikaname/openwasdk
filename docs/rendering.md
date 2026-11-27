# Rendering

`opencomp-core` turns a `Visual` tree into pixels. It holds no WinRT: `opencomp` marshals ABI
calls into the same tree, and one `Renderer` draws it on Metal, Vulkan or D3D12 through wgpu.
Nothing in the renderer names a window or a backend.

## The shape of a frame

Each frame flattens the root `Visual` (`flatten_root`) into a `Vec<Draw>`: one record per visible
rectangle, in draw order — parent first, children in collection order, later children on top. A
`Draw` carries four canvas-space corners, a scissor and a straight RGBA colour; flattening
composes the transform from the root down, multiplies opacity into the colour's alpha and
intersects the clip into the scissor. A visual is skipped when its size, effective alpha or
colour alpha is zero; a draw whose scissor misses the target is dropped.

`Renderer::render` builds one vertex buffer for the whole frame — six vertices, two triangles per
`Draw`, clip-space position plus colour per vertex — then issues one `draw` per rectangle with the
scissor set before it. No index buffer, no uniform, no bind group, no depth attachment: one
pipeline, one render pass, buffer rebuilt every frame. The pass opens with `LoadOp::Clear` to the
renderer's `background`.

## Colour

The core works in linear light. `Visual.color` is straight (unpremultiplied) linear RGBA; the
shader forwards it and the blend state is `ALPHA_BLENDING`, which blends in linear light because
the target format is an `*Srgb` one.

sRGB exists only at the ABI. `opencomp` decodes `Windows.UI.Color` with `opencomp_core::srgb` on
the way in and encodes with `srgb_from_linear` on the way out. Skipping the decode is visible:
byte 128 is 0.502 sRGB, which is 0.216 linear, so writing 0.5 into an sRGB target stores byte 188
where the client asked for 128. Colours built in the core are decoded the same way, which is what
`srgb` does in the examples. The background is the exception: `LoadOp::Clear` takes the floats as
linear, so a host passes `[srgb(0.09), ...]`, not `[0.09, ...]`.

## Opacity

`opacity` is a multiplier carried down the tree: the alpha at a vertex is `color.a * opacity of
self * opacity of each ancestor`. That is arithmetic on the colour, not a layer, so a translucent
parent does not composite its children as a group; overlapping translucent children show through
each other and the seams of such a group fade more than its interior.

Through the ABI each node carries only its own opacity: the snapshot copies `Node.opacity` and the
core does the multiplication, so two half-opaque ancestors leave a child at 0.25.

## Clip

`Visual.clip` is an axis-aligned rectangle in the node's own coordinates and clips its **children**,
not its own rectangle; the node's own rectangle is bounded by the clip inherited from its ancestors.
A scissor is the only clip mechanism, which fixes what clipping can mean:

| transform | clipping |
| --- | --- |
| translation, scale | exact; the clip rect is transformed and intersected |
| rotation | **not applied** to the rotated draw; it is bounded by its own axis-aligned bounds |
| nested clips | intersected, which stays a rectangle |

The rotation case is the limitation at the top of `tree.rs`; 90-degree multiples get the same
treatment although they would be expressible, costing a looser scissor and never a wrong pixel.
The scissor rounds outwards to whole device pixels and clamps to the target, so it may pass extra
pixels but never cuts into the quad.

## Transforms

A visual is placed by its offset, then rotated and scaled about its centre point:

```
world = parent * translate(offset) * translate(center) * rotate(rotation)
                * scale(scale) * translate(-center)
```

An `Affine` stores six f32, its third row always `[0, 0, 1]`. `offset`, `scale` and `center`
arrive as `Vector3` and lose `z`: no perspective, no 3D rotation, no transform matrix, no anchor
point. A child's offset and size are in its parent's transformed space.

## Device pixel ratio

Composition units are device independent; `size()` is in device pixels. `set_scale` takes the
host's scale factor (the example uses the window's `scale_factor`) and is applied last, after the
affine transform, both when vertices are built (`x * scale / width * 2 - 1`) and when the scissor
is computed. Layout never sees it.

## Formats, backends, surfaces

- `Renderer::new` — an existing device and queue, `Rgba8UnormSrgb`.
- `Renderer::with_format` — a host surface format, `Bgra8UnormSrgb` included.
- `Renderer::headless` — owns a device and draws into textures; tests and offscreen work.
- `render(&TextureView)` — the target must be `size()` and `format()`.
- `render_to_surface(&SurfaceTexture)` — the host acquired and configured the swapchain and
  presents the frame itself.
- `render_to_pixels()` — offscreen render and readback, row-major RGBA8, empty on failure; rows
  are padded to 256 bytes and the padding stripped.

No MSAA (`MultisampleState::default()` is one sample), no depth buffer, no timestamp query.

## What is missing

The renderer has one primitive: a solid rectangle, axis-aligned or transformed. So:

| feature | state |
| --- | --- |
| text | absent |
| bitmaps, composition surfaces | absent; `CreateSurfaceBrush` is `E_NOTIMPL` |
| rounded corners, ellipses, paths | absent; the geometry factories are `E_NOTIMPL` |
| effects, shadows, lights, backdrop | absent |
| layers, masks, gradients, nine-grid, 3D | absent |
| anti-aliasing | none of its own: hard triangle edges, one sample |

A rotated or fractional rectangle therefore has jagged edges, its corner pixels being the
rasteriser's rule rather than a coverage value computed here. `opencomp` accepts the properties it
cannot draw (anchor point, transform matrix, 3D rotation, composite mode, orientation) and drops
them: the tree stays consistent, the pixels do not change. A few setters are stricter than that and
throw `E_NOTIMPL` instead, `ISpriteVisual2.Shadow` and `IVisual.Parent` among them.

## `Visual` fields

| field | meaning | default |
| --- | --- | --- |
| `offset: [f32; 2]` | translation from the parent's origin | `[0.0, 0.0]` |
| `size: [f32; 2]` | rectangle drawn at local `(0, 0)`; zero draws nothing | `[0.0, 0.0]` |
| `color: [f32; 4]` | straight RGBA in linear light | `[0.0, 0.0, 0.0, 0.0]` |
| `opacity: f32` | multiplier for self and children | `1.0` |
| `scale: [f32; 2]` | scale about `center`, in local coordinates | `[1.0, 1.0]` |
| `rotation: f32` | radians about `center` | `0.0` |
| `center: [f32; 2]` | pivot in local coordinates | `[0.0, 0.0]` |
| `clip: Option<(f32, f32, f32, f32)>` | `x, y, width, height` clip for children | `None` |
| `children: Vec<Visual>` | drawn after the parent, later ones on top | empty |

## How to see it

```sh
cargo run -p opencomp-core --example scene -- /tmp/scene.ppm
cargo run -p opencomp-core --example window
OPENCOMP_FRAMES=3 cargo run -p opencomp-core --example window
```

`examples/scene.rs` writes a 320×240 tree with a clip and a translucent row as a PPM
(`render_to_pixels`); `examples/window.rs` puts the same tree in a winit window on the swapchain
path (`render_to_surface`), resize, format choice and scale factor included. Without a GPU adapter
both say so instead of failing.
