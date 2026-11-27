# OpenWASDK

An open SDK for WinUI 3 and Windows App SDK applications outside Windows:
**build them anywhere, and run existing binaries on Linux**.

> Status: technical validation. `comptrace` works, `opencomp` builds a visual
> tree through its ABI, and `opencomp-core` draws that tree on a GPU; no WinUI
> application runs on it yet.
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

Wine replaces the bottom row and runs the closed DLLs unmodified. OpenWASDK
replaces the two closed DLLs in the middle instead, so the compositor and the
XAML engine stop depending on DWM and DirectComposition. The application, its
compiled XAML and the Wine host stay as they are.

| Component | Role | State |
| --- | --- | --- |
| `comptrace` | Read WinRT metadata, generate bindings and implementation traits, map a traced IID back to its interface | works |
| `opencomp` | `Microsoft.UI.Composition` replacement: ABI-compatible WinRT server | visuals, brushes, inset clips |
| `opencomp-core` | Portable visual tree and wgpu renderer, no WinRT | draws trees, offscreen or on a swapchain |
| `openxaml` | `Microsoft.ui.xaml.dll` replacement: XBF loader, property system, layout, controls | planned |
| `openhost` | App model, `Microsoft.UI.Content`/`Windowing` hosting, activation, PE/WinRT host glue | planned |
| `xwinui` | Cross-compile, package and deploy WinUI 3 applications | planned |

The design and the measured scale are in [`docs/architecture.md`](docs/architecture.md).
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
```

`interfaces` prints the IID and method count of every interface, which is the
list a proxy has to answer. `methods` prints the signatures of an interface or a
namespace. `lookup` maps an IID from a trace back to its interface, or reports
that the metadata does not know it — the signal for a private interface. `diff`
reports what moved between two SDK versions. `classes` prints what each runtime
class implements, including its static interfaces.

## Regenerating the bindings

`opencomp` compiles a generated module, committed under
`crates/opencomp/src/bindings.rs` so a checkout builds without the SDK:

```sh
cargo run -p comptrace -- generate \
    path/to/Microsoft.UI.winmd path/to/Microsoft.Graphics.winmd \
    path/to/Microsoft.Foundation.winmd path/to/Windows.winmd \
    --namespace Microsoft.UI.Composition,Windows.UI.Composition,Windows.UI.Color,Windows.Foundation.Numerics \
    --minimal \
    --out crates/opencomp/src/bindings.rs
```

`--minimal` omits the client-side class wrappers; the runtime needs the
interfaces and the `*_Impl` traits only. `Windows.Foundation.Numerics` has to be
in the filter: without it the generator maps vectors to `windows-numerics`, which
has no `Quaternion`, so `IVisual.Orientation` is skipped and every `_Impl` trait
that requires `IVisual` disappears with it.

## Loading it on Windows

The DLL is three entry points — `DllGetActivationFactory`, `DllGetClassObject`,
`DllCanUnloadNow` — so it loads like any WinRT server. A client that knows the
class name can call `RoGetActivationFactory` with a manifest entry:

```xml
<file name="opencomp.dll">
  <activatableClass name="Microsoft.UI.Composition.Compositor"
                    threadingModel="both" />
</file>
```

inside the `<file>`/`<activatableClass>` group of an application manifest, with
the DLL next to the executable. Without the manifest, call
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
docs/                  architecture, measured scale, binary anatomy, milestones
```

## Roadmap

1. `comptrace` — metadata reports and the generator (done).
2. `opencomp` — first frame: `Compositor`, `ContainerVisual`, `SpriteVisual`
   on wgpu.
3. **Composition-only apps** — a program that hosts a visual tree in an HWND
   through `Microsoft.UI.Content`/`Microsoft.UI.Windowing` and never loads XAML.
   This is the first target that can run end to end, and the smallest one.
4. `openxaml` — XBF loader and the XAML runtime on top of the same compositor.
   Reactor (`microsoft/microsoft-ui-reactor`) is the first XAML-runtime target:
   it builds WinUI controls from C# and loads no application XBF, only the
   framework's own resources.
5. `openhost` — run without the Microsoft runtime DLLs.
6. `xwinui` — cross-compilation, packaging and deployment.

## License

MIT.
