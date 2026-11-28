# OpenWASDK

An open SDK for WinUI 3 and Windows App SDK applications outside Windows:
**an ABI-compatible runtime in Rust, plus the tools to build and preview them anywhere.**

> Status: the composition surface is the part that works. `opencomp` answers 58 interface
> types and 232 of 326 methods of `Microsoft.UI.Composition`; `xbf` parses the XBF node
> stream; `openxaml` lays out a first element tree and matches the real engine 38 px on a
> StackPanel. No Wine patches are part of the build: Wine was the measurement host for the
> ABI and is no longer on the critical path.
>
> Not affiliated with Microsoft.

## What it does

A WinUI 3 application sits on two closed runtimes, which sit on Windows:

```
app.exe + resources.pri + *.xbf
├── Microsoft.UI.Xaml.dll            open source, Windows-only build
│   └── Microsoft.UI.Xaml.Internal   closed
├── Microsoft.UI.Composition         closed
└── Windows: DirectComposition, DWM, CoreMessaging, app model, DWriteCore
```

A Windows host replaces the bottom row and runs the closed DLLs unmodified. OpenWASDK
replaces the two closed DLLs in the middle instead, so the compositor and the XAML engine
stop depending on DirectComposition or DWM, and the runtime is the same DLLs on Windows,
macOS, or any other host.

| Component | Role | State |
| --- | --- | --- |
| `comptrace` | Read WinRT metadata, generate bindings and implementation traits, map a traced IID back to its interface | works |
| `opencomp` | `Microsoft.UI.Composition` replacement: ABI-compatible WinRT server, and the `Microsoft.UI.Content` host | 58 interface types, 326 methods (197 with behaviour), D3D11/D2D presentation, animation, surfaces |
| `opencomp-core` | Visual tree, draw list and animation sampler; wgpu renderer behind a `preview` feature | trees with transforms, clip, opacity, bitmaps, rounded corners; previews offscreen or on a swapchain |
| `xbf` | XBF reader, ported from the MIT sources: header, metadata tables, node stream | parses the theme resources WinUI ships |
| `openxaml` | `Microsoft.ui.xaml.dll` replacement: XBF loader, property system, layout, controls | element model, layout and a composition tree; no controls yet |
| `openudk` | `Microsoft.Internal.FrameworkUdk.dll` stand-in: the flat C layer XAML imports | 22 of 149 exports implemented, the rest stubs with a stated reason |
| `openhost` | App model, `Microsoft.UI.Windowing` hosting, PE host glue | Content hosting and activation live in `opencomp`; the app model is planned |
| `xwinui` | Cross-compile, package and deploy WinUI 3 applications | SDK layout, a 1033-class activation manifest generated from the SDK's own MSIX, `dotnet` build through the SDK's XAML compiler on Windows; not yet exercised end to end |

## How much of it is implemented

There are two paths to running WinUI 3 here, and a number without its path is a
misleading number:

- **Gates B-D**: Microsoft's own `Microsoft.ui.xaml.dll` runs on top of our layers
  (`opencomp`, `opendispatch`, `openudk`). Every XAML runtime class is Microsoft's code;
  our obligation is the ABI underneath it.
- **Milestone 6**: `openxaml` replaces that DLL. There we implement the classes.

### The layers beneath the official engine

Counted from the code: the `*_Impl` methods behind every `#[implement(..)]` type, minus
the ones that answer `E_NOTIMPL`; denominators from the metadata in
[`docs/winui-anatomy.md`](docs/winui-anatomy.md).

| surface | implemented | of | |
| --- | ---: | ---: | ---: |
| XAML runtime classes hosted | 900 | 900 | **100%** (Microsoft's code, and it starts) |
| Composition ABI methods | 232 | 326 | **71%** |
| `Microsoft.UI.Dispatching` classes | 5 | 5 | **100%** |
| FrameworkUdk C exports | 22 | 149 | **15%** |
| App SDK classes with an activation entry | 1033 | 1033 | **100%** (generated) |
| XBF node kinds mapped to an element tree | 30 | 39 | **77%** |
| XBF value kinds | 11 | 11 | **100%** |
| Acceptance gates | A, Gallery window under our `dcompi` | A-D | **2** |
| Tests | 186 | - | - |

Fifty-eight composition interface types are implemented; the engine imports 73, and the
intersection is not measurable without the binary, so no percentage is claimed for it.
The 94 composition methods that still answer `E_NOTIMPL` are listed with the subsystem
each needs in [`docs/opencomp-stubs.md`](docs/opencomp-stubs.md): lights and effects need
a second renderer path, expressions need an evaluator, commits needed a clock (being
added), z-order belongs to the host window manager.

### Replacing the engine: `openxaml`

| surface | implemented | of | |
| --- | ---: | ---: | ---: |
| XAML element kinds | 4 | 91 Reactor builds | **4%** |
| XAML runtime classes | 4 | 900 | **0.4%** |
| Dependency properties | 18 | - | - |

Those four are enough to check our layout rules against the real engine's numbers --
the first such check says 38 px on both sides -- but they are not enough to run an
application, and they do not need to be until milestone 6. Until then the 900 classes
come from Microsoft's DLL, which is what the row above means by "hosted": the engine is
theirs and the layers beneath it are ours.

The 186 tests run on Windows and macOS.

The design and the measured scale are in [`docs/architecture.md`](docs/architecture.md).
What this project does **not** do — the boundary against Wine, DXVK and
`wine-uwp` — is in [`docs/scope.md`](docs/scope.md), with the acceptance ladder
that ends at running WinUI 3 Gallery unmodified.
What the shipped App SDK binaries contain — layers, interface counts, the private
boundary — is in [`docs/winui-anatomy.md`](docs/winui-anatomy.md).

## Requirements

- Rust 1.95 or newer.
- For code generation, the WinRT metadata from NuGet (no Windows needed):
  - `Microsoft.WindowsAppSDK.InteractiveExperiences` (2.7.0):
    `metadata/10.0.18362.0/Microsoft.UI.winmd`, `Microsoft.Graphics.winmd`,
    `Microsoft.Foundation.winmd`
  - `Microsoft.Windows.SDK.CPP` (10.0.26100.x): `c/UnionMetadata/10.0.26100.0/Windows.winmd`

## Using comptrace

```sh
cargo run -p comptrace -- summary    Microsoft.UI.winmd
cargo run -p comptrace -- interfaces Microsoft.UI.winmd --namespace Microsoft.UI.Composition
cargo run -p comptrace -- methods    Microsoft.UI.winmd --interface IVisual
cargo run -p comptrace -- classes    Microsoft.UI.winmd
cargo run -p comptrace -- lookup     Microsoft.UI.winmd 5ac4e92e-017b-5d68-8198-f68636ab99d3
cargo run -p comptrace -- diff       old/Microsoft.UI.winmd new/Microsoft.UI.winmd
cargo run -p comptrace -- attributes Microsoft.UI.winmd Microsoft.UI.Composition.Compositor
cargo run -p comptrace -- iids       Microsoft.ui.xaml.dll --metadata Microsoft.UI.winmd
cargo run -p comptrace -- imports    Microsoft.ui.xaml.dll --dll frameworkudk
cargo run -p comptrace -- exports    opencomp.dll
cargo run -p comptrace -- strings    Microsoft.ui.xaml.dll --prefix Microsoft.UI.Content. --wide
```

For a client, `generate --client --namespace-file names.txt` emits class wrappers, activation and
static factories and the `_Impl` traits; `--check` compares the result with `--out` instead of
writing it, so a stale committed `bindings.rs` fails without being regenerated by hand.

`interfaces` prints the IID and method count of every interface, which is the
list a proxy has to answer. `methods` prints the signatures of an interface or a
namespace. `lookup` maps an IID from a trace back to its interface, or reports
that the metadata does not know it — the signal for a private interface. `diff`
reports what moved between two SDK versions. `classes` prints what each runtime
class implements, including its static interfaces. `iids` goes the other way:
it scans a built binary for the identifiers it loads and names them against the
metadata, which is how a version bump is audited from the client side. `imports`
and `exports` read the PE tables, so a dependency question ("which names does
XAML take from this DLL?") is answered without a Windows host. `strings` answers
which DLL carries a class name, wide or narrow — the quickest way to find which
module serves a WinRT class.

## Regenerating the bindings

`opencomp` compiles a generated module, committed under
`crates/opencomp/src/bindings.rs` so a checkout builds without the SDK:

```sh
cargo run -p comptrace -- generate \
    path/to/Microsoft.UI.winmd path/to/Microsoft.Graphics.winmd \
    path/to/Microsoft.Foundation.winmd path/to/Windows.winmd \
    --namespace Microsoft.UI.Composition,Windows.UI.Composition,Windows.UI.Color,Windows.Foundation.Numerics,Microsoft.UI.Content,Microsoft.UI.Composition.Visual,Microsoft.UI.Composition.Compositor,Microsoft.UI.WindowId,Microsoft.UI.DisplayId,Microsoft.UI.Dispatching,Windows.Foundation.TypedEventHandler,Windows.Foundation.Collections,Windows.Foundation.IClosable,Windows.Graphics.RectInt32,Windows.Graphics.SizeInt32,Windows.Graphics.PointInt32,Windows.Foundation.Size,Microsoft.Graphics.DirectX.DirectXAlphaMode,Microsoft.Graphics.DirectX.DirectXPixelFormat \
    --minimal \
    --out crates/opencomp/src/bindings.rs
```

`--minimal` omits the client-side class wrappers, except for classes named
explicitly in the filter: `Microsoft.UI.Composition.Visual` and `Compositor` are
there because the Content staging uses those exact projections, and the
`Microsoft.UI.Content` classes come with the namespace. `Windows.Foundation.Numerics`
has to be in the filter: without it the generator maps vectors to
`windows-numerics`, which has no `Quaternion`, so `IVisual.Orientation` is skipped
and every `_Impl` trait that requires `IVisual` disappears with it. Do not add
`Windows.Foundation` itself: the generator then emits references to
`Windows.Storage`, `Windows.Devices` and `Windows.Security` types that the filter
does not provide, and the module no longer compiles.

windows-bindgen 0.100 emits the `TypedEventHandler` delegate box private and
refers to it unqualified from the middle namespaces. After generating, run:

```sh
python3 - <<'EOF'
p = "crates/opencomp/src/bindings.rs"
s = open(p).read()
s = s.replace("&TypedEventHandlerBox::<",
              "&crate::bindings::Windows::Foundation::TypedEventHandlerBox::<")
s = s.replace("& TypedEventHandlerBox::<",
              "& crate::bindings::Windows::Foundation::TypedEventHandlerBox::<")
s = s.replace("        struct TypedEventHandlerBox<",
              "        pub(crate) struct TypedEventHandlerBox<")
s = s.replace("            const VTABLE: TypedEventHandler_Vtbl<TSender, TResult> =",
              "            pub(crate) const VTABLE: TypedEventHandler_Vtbl<TSender, TResult> =")
open(p, "w").write(s)
EOF
```

The box and its `VTABLE` become `pub(crate)`, and every use is qualified with the
crate path so the generated file compiles as one module.

`Windows.Graphics` as a whole is not in the filter: it pulls `Windows.Storage`
and the whole device stack with it. The four types the drawing surface names are
listed one by one instead, which is also why `Microsoft.Graphics.DirectX` appears
twice: the namespace and the two enums.

## Loading it on Windows

The DLL is three entry points — `DllGetActivationFactory`, `DllGetClassObject`,
`DllCanUnloadNow` — so it loads like any WinRT server. A client that knows the
class name can call `RoGetActivationFactory` with a manifest entry:

```xml
<file name="opencomp.dll">
  <activatableClass name="Microsoft.UI.Composition.Compositor"
                    threadingModel="both"
                    xmlns="urn:schemas-microsoft-com:winrt.v1" />
</file>
```

inside the `<file>`/`<activatableClass>` group of an application manifest, with
the DLL next to the executable. The namespace is not decoration: without it both
the Windows SxS parser and Wine read the document as an assembly with no
activatable classes, and activation fails with `REGDB_E_CLASSNOTAVAILABLE`. Without the manifest, call
`DllGetActivationFactory` yourself and `QueryInterface` for
`IActivationFactory`; `crates/opencomp/src/factory.rs` is the same code from the
other side.

## Building

```sh
cargo build
cargo test
cargo check -p opencomp --target x86_64-pc-windows-msvc
```

The package target is a Windows DLL (`cdylib`). Checking it from Linux or macOS
validates the ABI code; linking needs a Windows toolchain.

`opencomp-core` has no WinRT in it, so it builds everywhere and its tests render
through whatever backend the host has: Metal on macOS, Vulkan or D3D12
elsewhere. A machine without a GPU skips those tests.

## Seeing it draw

Two examples use no WinRT at all, so they run on any of the three hosts:

```sh
# a tree with a clip and a translucent row, written as a PPM
cargo run -p opencomp-core --example scene -- /tmp/scene.ppm

# the same tree in a window, on the swapchain path the ABI will present through
cargo run -p opencomp-core --example window

# for a machine with nobody watching, exit after three frames and report the format
OPENCOMP_FRAMES=3 cargo run -p opencomp-core --example window
```

`opencomp` builds the same tree from the other side: a client activates the
class, calls `CreateContainerVisual` and `CreateSpriteVisual`, inserts children
and sets a clip, and the shell hands the result to this renderer. Its tests do
exactly that, which is why they need a Windows host to run.

## Layout

```
crates/comptrace       metadata reader, reports and the binding generator
crates/opencomp        Microsoft.UI.Composition replacement (Windows target)
crates/opencomp-core   portable visual tree and wgpu renderer (every host)
crates/openudk         Microsoft.Internal.FrameworkUdk.dll stand-in (flat C exports)
docs/                  architecture, measured scale, binary anatomy, milestones
```

## Roadmap

1. `comptrace` — metadata reports and the generator (done).
2. `opencomp` — first frame: `Compositor`, `ContainerVisual`, `SpriteVisual`
   presented through D3D11 and D2D (done; `wgpu` remains as a preview backend).
3. **Composition-only apps** — a program that hosts a visual tree in an HWND
   through `Microsoft.UI.Content` and never loads XAML (done: Gate A).
4. `openxaml` — the official `Microsoft.ui.xaml.dll` first, on top of this
   compositor (Gate B), then our own XAML runtime built from XBF.
   Reactor (`microsoft/microsoft-ui-reactor`) is the first XAML-runtime target:
   it builds WinUI controls from C# and loads no application XBF, only the
   framework's own resources.
5. `openhost` — run without the Microsoft runtime DLLs.
6. `xwinui` — cross-compilation, packaging and deployment.

## License

MIT.
