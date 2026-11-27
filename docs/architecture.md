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
- Renderer: wgpu (Vulkan/Metal/D3D12) with Skia for text and vector work. The
  same core runs native on Linux/macOS for tooling and tests.
- No DWM and no DirectComposition: presentation goes straight to a window
  (HWND under Wine, native surfaces elsewhere).
- D3D interop: XAML hands over D3D textures through composition surfaces; those
  must be consumable by the renderer (Wine's d3d11 or a conversion to wgpu).

Language: Rust. The ABI shell is a `cdylib` built for
`x86_64-pc-windows-gnu` (loadable by Wine); the compositor is plain Rust with
wgpu. `windows`/`windows-bindgen` provide the ABI types; C++/WinRT is only a
fallback if a private interface turns out to need hand-written vtables.

The renderer lives in `opencomp-core`, which contains no WinRT. That split is
what makes macOS a first-class host for the compositor: the same visual tree
renders through Metal on macOS, Vulkan on Linux and D3D12 elsewhere, and its
tests run on every machine with a GPU. The Windows shell only marshals ABI
calls into the core.

## openxaml

`Microsoft.UI.Xaml.Internal.dll` replacement.

- XBF loader: parse `resources.pri` and the embedded XBF into an object graph.
  References: the WinUI OSS `dxaml` sources (MIT), `XbfDump`, `XbfDecompiler`,
  `XbfKit` for format validation.
- Runtime: dependency properties, styles/templates, resources, bindings,
  layout, input routing, focus and accessibility.
- Controls: WinUI's control set, ported against the same XAML semantics. Uno's
  WinUI-compatible implementation is the best behavioural reference; WPF is the
  reference for the property/binding system.
- Text: Skia paragraph or HarfBuzz + ICU; DirectWrite is not required.

`Microsoft.UI.Xaml.dll` itself can come from the OSS WinUI sources (MIT) once
the build is portable; until then the official binary can be used, because the
contract between it and `Internal.dll` is what `openxaml` implements.

## openhost

The glue that makes the two replacements loadable and runnable:

- app model: package identity, activation, `CoreApplication`/`CoreWindow`
  equivalents, protocol activation;
- PE/WinRT host: manifests and `activatableClass` registration, or runtime
  factory registration through `RoRegisterActivationFactories`;
- windowing and input plumbing, including `Microsoft.UI.Windowing` and
  `Microsoft.UI.Input` if they are not replaced outright.

Under Wine this is an addition to the platform layer (thread, window, message
pump); on a native port it becomes a small PE/WinRT loader of its own.

## xwinui

The build half, independent from the runtime:

1. **SDK** — `xwin` for the Windows SDK and CRT; a WinAppSDK extractor for the
   NuGet packages and their `.winmd`/projection headers.
2. **Compile** — `clang-cl` + `lld-link` for C++/WinRT, `dotnet` with
   `EnableWindowsTargeting` for C#.
3. **Package** — `makemsix` and `osslsigncode`; generated `AppxManifest.xml`
   and a self-signed certificate chain.
4. **Run** — push to a Windows machine over SSH, or install into a Wine prefix
   running the OpenWASDK runtime; stream the debug output back.

The first milestone is to build a plain WinUI 3 app on Linux and run it under
Wine with the official runtime. That validates the toolchain before any
runtime code exists, and it produces the test binaries `comptrace` needs.

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

## Milestones

| # | Milestone | Exit criterion |
| --- | --- | --- |
| 0 | `comptrace` | A proxy logs WinUI 3 Gallery's Composition calls and produces the interface list |
| 1 | `xwinui` | An unmodified WinUI 3 sample builds on Linux and runs under Wine with the official runtime |
| 2 | `opencomp` | A Composition-only program creates a visual tree on our DLL and draws it |
| 3 | composition-only host | The same program hosts its tree in an HWND through `Microsoft.UI.Content`/`Windowing`, with no XAML loaded |
| 4 | `openxaml` | A WinUI 3 sample runs without `Microsoft.UI.Xaml.Internal.dll` |
| 5 | `openhost` | No Microsoft runtime DLL remains; only Wine's platform layer |

## Risks

- **Private Composition interfaces.** The proxy finds them; their semantics need
  disassembly. This is the schedule risk, not the line count.
- **D3D interop.** XAML assumes it can hand D3D resources to the compositor.
- **Version binding.** Composition's ABI moves with WinAppSDK; pin one version.
- **XBF format.** Undocumented; validate against real applications early.
- **Legal.** Replacements must be clean-room where the interface is discovered
  by reverse engineering: interfaces may be identified (for interoperability),
  implementations are written from behavior and public documentation.
