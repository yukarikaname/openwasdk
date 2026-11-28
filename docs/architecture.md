# Architecture

## The stack we are replacing

WinUI 3 is a WinRT surface on top of a Windows-only runtime. The pieces, from
the app down:

```
app.exe
  resources.pri, *.xbf  (compiled XAML)
  Microsoft.UI.Xaml.dll          MIT source, Windows-only build
  Microsoft.UI.Xaml.Internal.dll closed
  Microsoft.UI.Composition.dll   closed
  Microsoft.UI.Input.dll         closed
  Microsoft.UI.Windowing.dll     closed
  ---------- Windows ----------
  DirectComposition (dcomp), DWM, CoreMessaging, app model, DWriteCore, D3D
```

Two boundaries can be replaced without touching the application:

1. **platform boundary** — below everything. Replacing it means Wine: user32,
   combase, d3d, dwrite, app model. `wine-uwp` and `wine-ultimate` do this.
2. **framework boundary** — the two closed DLLs in the middle. Replacing them
   means writing an ABI-compatible `Microsoft.UI.Composition` and
   `Microsoft.UI.Xaml.Internal`.

OpenWASDK works on boundary 2 and reuses a Wine host for boundary 1. An
application binary and its XBF do not change; only the two DLLs do.

## comptrace

Purpose: turn "what does the closed XAML binary call?" into a finite list.

- Read the WinAppSDK `.winmd` (`Microsoft.UI.Composition.winmd`,
  `Microsoft.UI.Xaml.winmd`, `Microsoft.UI.Input.winmd`, …) with
  `windows-bindgen` / `cppwinrt` and generate:
  - a **forwarding proxy** with the same class names, IIDs and vtable order that
    loads the real DLL and logs each call, and
  - a **stub server** skeleton for `opencomp`/`openxaml` with the same ABI.
- Log activations, `QueryInterface` IIDs and method calls from a real app
  (WinUI 3 Gallery is the reference app), then diff the IIDs against the
  `.winmd`.
- Interfaces that appear in the log but not in the metadata are the private
  surface; those are the only places that need a disassembler
  (IDA/Ghidra/Binary Ninja) and the public symbol server.

The proxy is useful outside this project: it is a WinRT call tracer.

## opencomp

`Microsoft.UI.Composition.dll` replacement.

- WinRT server: `DllGetActivationFactory` plus one `#[implement]` type per
  runtime class, generated from the `.winmd` so IIDs and vtable order match.
- Compositor: visual tree (`ContainerVisual`, `SpriteVisual`), brushes and
  surfaces, animations and implicit animations, effects and backdrops, commit
  and transaction semantics, `CompositionTarget`, device loss.
- Renderer: **D3D11 and D2D1**, the same APIs the real stack uses and the same
  device the client hands over. The core (tree, flatten, animation sampling, draw
  list) is GPU-agnostic; wgpu exists only behind a `preview` feature for looking at
  a tree on the machine you are on, and the DLL never enables it.
- Text and vector work are D2D1's, not ours: `Microsoft.ui.xaml.dll` imports
  `d2d1.dll` and `DWrite.dll` itself.
- No DWM and no DirectComposition: presentation goes straight to a window. The
  window is always a Win32 one, so the process is a Windows process: under Wine
  on Linux and macOS, or on Windows itself. The only native surfaces are in our
  own preview tooling, which draws a tree with wgpu on the host without running
  any Windows binary.
- D3D interop: XAML hands over a D3D11 device and draws into surfaces on it; the
  compositor draws those textures on the same device, with no copy and no
  cross-API import.

Language: Rust. The ABI shell is a `cdylib` built for
`x86_64-pc-windows-gnu` (loadable by Wine); the compositor is plain Rust with
wgpu. `windows`/`windows-bindgen` provide the ABI types; C++/WinRT is only a
fallback if a private interface turns out to need hand-written vtables.

What the DLL answers today, and what it reports `E_NOTIMPL` for:

| Implemented | Answers `E_NOTIMPL` |
| --- | --- |
| `CreateContainerVisual`, `CreateSpriteVisual`, `CreateColorBrush`, `CreateColorBrushWithColor`, `CreateInsetClip`, `CreateInsetClipWithInsets`, `CreateLinearEasingFunction`, `CreateCubicBezierEasingFunction`, `CreateScalarKeyFrameAnimation`, `CreateVector2KeyFrameAnimation`, `CreateVector3KeyFrameAnimation`, `CreateColorKeyFrameAnimation`, `CreatePropertySet`, `CreateStepEasingFunction`, `CreateStepEasingFunctionWithStepCount`, `CreateAnimationGroup`, `CreateScopedBatch`, `GetCommitBatch`, `RequestCommitAsync`, `CreateAnimationController` | implicit animations, surfaces, effects, shadows, lights |
| `IContainerVisual.Children` with insert, remove and count | the rest of `VisualCollection`'s iteration |
| `IVisual.Offset`, `Size`, `Opacity`, `IsVisible`, `Clip`, `Scale`, `RotationAngle`, `RotationAngleInDegrees`, `CenterPoint`, `AnchorPoint` | transform matrix, orientation, border mode, backface, composite mode |
| `ISpriteVisual.Brush`, `ICompositionColorBrush.Color`, `IInsetClip` insets, easing control points and steps, `ICompositionPropertySet`, `ICompositionAnimationGroup`, `StartAnimation`/`StartAnimationGroup`/`StopAnimation` on visuals, `ICompositionAnimation2.Target` | `RectangleClip` and geometric clips, expression animations, implicit animations, `TryGetAnimationController`, `StopAnimationGroup` |
| every object answers `ICompositionObject2`/`4`/`5`, visuals also `IVisual2`/`3`/`4`, sprites `ISpriteVisual2`, clips `ICompositionClip2` | the methods behind them |

The follow-on interfaces exist as stubs on purpose: a client asks for them with
`QueryInterface` before it calls anything, and a failed QI throws, while a method
that returns `E_NOTIMPL` can be handled.

### The flat C layer

`Microsoft.ui.xaml.dll` statically imports `Microsoft.Internal.FrameworkUdk.dll`,
a 441 KB DLL with 149 plain C exports, so it cannot be loaded unless that DLL is
found. XAML links 43 of them; six `Windowing_*` exports (WindowId ↔ HWND and the
display/icon conversions) are the structural part, and the rest are optional
features that fail cleanly. `dcompi.dll` links ten: the `DCompPrivates_*` shared
visual handoff and `DxPrivates_*`.

That layer is small, has no metadata and no COM, and is therefore the cheapest
place to start against a real XAML binary: a crate that exports those names,
with the conversions implemented and the rest returning `E_NOTIMPL`, is enough
for the loader and for XAML's first HWND lookups.

### Hosting

A composition tree reaches a window through `Microsoft.UI.Content`, not through
Composition itself. `DesktopChildSiteBridge.Create(Compositor, WindowId)` takes
the HWND as a `WindowId`, the bridge `Connect`s a `ContentIsland` that owns the
root `Visual`, and the island is observed through `ContentSite` and
`ContentSiteView`. That namespace is 51 interfaces and 214 methods, so it is a
subsystem of its own rather than a shim.

The older route, `ICompositorDesktopInterop::CreateDesktopWindowTarget`, is a C++
header interop: it is not in any `.winmd`, so `comptrace` cannot see it, and a
client that uses it links the header's IID and calls into the DLL through a
`QI` we would have to answer anyway.

Both routes end at the same place: an HWND whose swapchain this project's
renderer can own (wgpu accepts a raw Win32 window handle), and whatever the
client drew into composition surfaces, which for XAML means D3D interop. The
tree we already build is the part that does not need either.

The first half of that is now in `opencomp`: `ContentIsland` and
`DesktopChildSiteBridge` are served from the same DLL, `Create` takes the root
visual and the window id, and the site carries size, visibility and z-order. The
island hands its root node to a host through `content::root_node`, so a preview
loop can snapshot it without any Content object knowing about wgpu. What is
missing is the frame itself: no swapchain is created, no message pump runs, and
`ContentSiteView`, events and `RequestSize` answer `E_NOTIMPL`.

Colour is linear in the core and sRGB at the ABI. A surface in an `*Srgb` format
stores encoded bytes and blends in linear light, so a brush's `Windows.UI.Color`
bytes are decoded on the way in (`opencomp_core::srgb`) and encoded on the way
out; writing 0.5 straight into an sRGB target produces byte 188, not 128.

The renderer lives in `opencomp-core`, which contains no WinRT. That split is
what makes macOS a first-class host for the compositor: the same visual tree
renders through Metal on macOS, Vulkan on Linux and D3D12 elsewhere, and its
tests run on every machine with a GPU. The Windows shell only marshals ABI
calls into the core.

## openxaml

`Microsoft.ui.xaml.dll` replacement. That DLL is the WinRT server for every XAML
class and exports only `DllGetActivationFactory`; `Microsoft.UI.Xaml.Internal.dll`
is the Line Services text engine, not the XAML ABI.

- XBF loader: parse `resources.pri` and the embedded XBF into an object graph.
  References: the WinUI OSS `dxaml` sources (MIT), `XbfDump`, `XbfDecompiler`,
  `XbfKit` for format validation.
- Runtime: dependency properties, styles/templates, resources, bindings,
  layout, input routing, focus and accessibility.
- Controls: WinUI's control set, ported against the same XAML semantics. Uno's
  WinUI-compatible implementation is the best behavioural reference; WPF is the
  reference for the property/binding system.
- Text: Skia paragraph or HarfBuzz + ICU; DirectWrite is not required.

The OSS WinUI sources (MIT) are the upstream of that DLL. Until they build
portably the official binary can be used, because the contract between it and
the compositor is only 73 interfaces plus 16 private identifiers.

### Reactor

`Microsoft.UI.Reactor` is a declarative C# authoring layer over WinUI 3: every
element it renders is a real WinUI control. It is not a XAML-free runtime, so it
still needs the XAML engine; what it does not need is the XAML
compiler or an application XBF, because the tree is built by C# calls.

That makes it the best first target for `openxaml`: a small, open-source corpus
of exactly which controls, properties and layout paths a modern app touches,
without a parser to write first. Running it also needs a .NET host under Wine,
which is a second dependency independent of the control set.

## openudk

`Microsoft.Internal.FrameworkUdk.dll` stand-in. It is not optional: the XAML DLL
links 43 of its exports statically, so a missing file is a load failure before
any of the code above runs. The crate exports all 149 names; the six `Windowing_*`
conversions are real (a `WindowId` is the `HWND` reinterpreted), the other 143
answer `E_NOTIMPL` from bodies that take no arguments, which is what makes one
stub answer every prototype on x86-64.

The build produces `openudk.dll`; packaging renames it.

## openhost

The glue that makes the two replacements loadable and runnable:

- app model: package identity, activation, `CoreApplication`/`CoreWindow`
  equivalents, protocol activation;
- PE/WinRT host: manifests and `activatableClass` registration, or runtime
  factory registration through `RoRegisterActivationFactories`;
- windowing and input plumbing, including `Microsoft.UI.Windowing` and
  `Microsoft.UI.Input` if they are not replaced outright.

A client reaches these classes through one registration or the other, never by
calling our export. A packaged or unpackaged application ships a manifest whose
`activatableClass` entries name the DLL — the document `xwinui manifest` writes,
embedded as `RT_MANIFEST` or placed at `<executable>.manifest` — so combase reads
it from the process activation context and loads the DLL itself. The entries must
be in the WinRT namespace (`urn:schemas-microsoft-com:winrt.v1`), and the DLL has
to be findable by the name the entry gives, which for a file name alone means
beside the executable. A host that loads us itself can instead call
`opencomp::register()` before its first activation, which registers the same names
in-process with `RoRegisterActivationFactories`; the runtime documents that call
for out-of-process exe servers and may refuse it for a plain host, so the manifest
is the route that always works. Wine's combase implements the manifest lookup but
stubs `RoRegisterActivationFactories`, so the Wine run exercises the manifest.

Under Wine this is an addition to the platform layer (thread, window, message
pump); on a native port it becomes a small PE/WinRT loader of its own.

## xwinui

The build half, independent from the runtime:

1. **SDK** — the Windows SDK and the MSVC CRT come from `xwin`, which already
   downloads and lays out both; `xwinui layout` only adds what `xwin` does not
   know about: the App SDK packages, their `.winmd`, their projection headers and
   their 31 metadata files. Downloading the SDK ourselves would be a second
   implementation of a solved problem (and the SDK package is 149 MiB before its
   import libraries, which live in a separate package).
2. **Compile** — `clang-cl` plus `lld-link` for C++/WinRT, `dotnet` with
   `EnableWindowsTargeting` for C#. MSVC's `cl.exe` does not run here and MinGW is
   not C++ ABI compatible with MSVC, so clang-cl is the only cross-compiler that
   can link against the official headers. Rust code that only crosses the ABI by
   C-style vtable — which is all of ours — builds with `x86_64-pc-windows-gnu`
   and needs none of that.
3. **Package** — `makemsix` and `osslsigncode`; generated `AppxManifest.xml`
   and a self-signed certificate chain.
4. **Run** — push to a Windows machine over SSH, or install into a Wine prefix
   running the OpenWASDK runtime; stream the debug output back.

Wine is needed for exactly two steps of a build, both Microsoft *tools* rather
than compilers: `XamlCompiler.exe` (markup to XBF, a .NET Framework PE) and
`makemsix`. Code without `.xaml` compiles and links with no Windows runtime at
all.

Building an application is not required to run one — the milestone binaries are
released — but a self-built WinUI Gallery is worth having once failures need
bisecting: symbols, logging, and the freedom to remove pages.

`xwinui` builds and packages; it does not run anything natively. A WinUI 3
application is a Windows PE that calls `user32`, `d3d11`, `d2d1`, `dwrite` and
`combase`, so "running" it always means Wine (or Windows), whatever we replace.

The toolchain is validated where the runtime is: `xwinui` builds a WinUI 3 app, and the
baseline VM runs it against the official runtime. Wine is no longer a target -- it is
frozen as a measurement host, so a build step that only Wine can run is a gap in
`xwinui`, not a reason to patch Wine.

## Measured scale

Counted with `comptrace summary` over the Windows App SDK 2.7.0 metadata
(`Microsoft.WindowsAppSDK.InteractiveExperiences`,
`metadata/10.0.18362.0/Microsoft.UI.winmd`):

| namespace | interfaces | methods |
| --- | --- | --- |
| Microsoft.UI.Composition | 177 | 709 |
| Microsoft.UI.Composition.Interactions | 42 | 138 |
| Microsoft.UI.Composition.Scenes | 29 | 76 |
| Microsoft.UI.Composition.SystemBackdrops | 10 | 41 |
| Microsoft.UI.Composition.Effects | 2 | 12 |
| Microsoft.UI.Composition.Core | 1 | 5 |
| Microsoft.UI.Composition.Diagnostics | 3 | 6 |
| **Composition family** | **264** | **987** |
| Microsoft.UI.Input | 71 | 298 |
| Microsoft.UI.Content | 51 | 214 |
| Microsoft.UI.Windowing | 28 | 155 |

The whole `Microsoft.UI.winmd` is 440 interfaces and 233 runtime classes;
`Microsoft.UI.Xaml.winmd` is 1860 interfaces and 900 classes. Wine's
DirectComposition IDL, for comparison, is 39 interfaces and 281 methods, so the
public Composition surface is about three and a half times dcomp before any
private interface is counted.

## The run loop

Everything Windows-shaped is verified on GitHub's runners or on the baseline VM:

| step | what it proves |
| --- | --- |
| `cargo test --workspace` on `windows-latest` | the ABI objects and the presenter work on a real Windows |
| `tests/gate_a.rs` there | a window, an island, a bridge, and at least one presented frame |
| the Gallery on the baseline VM | the engine runs on our `dcompi` shim and draws (`composition-baseline.md`) |
| mingw link + `objdump -p` | the PE has the three exports it must |

Anything that can hang is wrapped in `timeout`, so a wedged graphics call is a red
step with a log rather than a dead runner.

`OPENCOMP_TRACE=1` makes the presenter name each graphics stage before it runs, so a
stage that never returns prints its own name and nothing after it. The first frame
costs a device, a swapchain and a D2D target before anything is drawn, which is
16 ms on the Windows runner, so a wait for the first frame has to be seconds, not
frames.

The client links the compositor as a library *and* loads it as a DLL through combase,
which is two copies of the same statics. Anything a host wants to read back (the
frame counter, for one) has to come from the copy that did the work, which is why
`opencomp_frames_presented` is exported.

## Milestones

The binaries themselves are measured in [`winui-anatomy.md`](winui-anatomy.md):
73 Composition interfaces are all the XAML engine asks for, and 14 private IIDs
cross the boundary.

| # | Milestone | Exit criterion |
| --- | --- | --- |
| 0 | `comptrace` and symbols | Interface list and fault offsets resolved from the metadata and the symbol server, never from a guessed vtable slot (done) |
| 1 | `dcompi` shim | Our `dcompi.dll` in front of the engine and the Gallery window renders -- `WinUIDesktopWin32WindowClass`, title `WinUI Gallery` -- with no class taken over (done, 3b2e015) |
| 2 | class takeover | Each class moved into `OPENCOMP_MINE` keeps that window; a class that breaks it goes back, it is not patched blind |
| 3 | XBF to pixels off Windows | A page's `.xbf` loads, arranges and renders to a PNG on macOS through wgpu/Metal. Measured: 47 of the Gallery's 195 `.xbf` files materialise and render, up from 0. The rest stop on a control type the element table does not carry (a `ListView`, an `Image`, an `Ellipse`), or on a page that derives from a base class compiled into the app's DLL, whose body no `.xbf` carries |
| 4 | text and images in the preview | A `TextBlock`/`Image` page shows glyphs and bitmaps, compared against a screenshot of the same page on Windows |
| 5 | window and input off Windows | The same tree in a winit window, with hit-testing and scrolling |
| 6 | `openxaml` | A WinUI 3 application runs on our `Microsoft.ui.xaml.dll`, with no Microsoft XAML loaded |

Not a milestone: an unmodified WinUI Gallery -- a Windows PE and a .NET application --
running natively on macOS. It needs the engine, D3D and the lifted DWM; Wine was measured
not to supply them, so that target is a Windows VM, not this project. What
runs off Windows is the track above: our own `.xbf` reader, layout, compositor and
renderer. A source-level C# facade can also run managed WinUI code on macOS, and one
exists (`refs/winui3-mac-test-runtime`); it needs the application rebuilt against its
types, which is a different answer to a different question.

## Risks

- **Private Composition interfaces.** The proxy finds them; their semantics need
  disassembly. This is the schedule risk, not the line count.
- **D3D interop.** XAML assumes it can hand D3D resources to the compositor.
- **Version binding.** Composition's ABI moves with WinAppSDK; pin one version.
- **XBF format.** Undocumented; validate against real applications early.
- **Legal.** Replacements must be clean-room where the interface is discovered
  by reverse engineering: interfaces may be identified (for interoperability),
  implementations are written from behavior and public documentation.
