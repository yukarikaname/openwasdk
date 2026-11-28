# What this project owns

OpenWASDK replaces the App SDK's own DLLs — the ones that ship inside the
`Microsoft.WindowsAppSDK.*` NuGet packages and land in
`runtimes-framework/win-x64/native/`. Nothing else. Everything a Windows
installation or a host platform already provides is used as it is.

The test for a change: **if replacing our DLL with the official one on a real
Windows machine would make the change unnecessary, it is ours. Otherwise it
belongs to the host platform, and we do not write it.**

## Current direction

Two tracks, and neither of them patches Wine:

- **Runtime (B).** An ABI-compatible reimplementation of the App SDK's own
  components in Rust: `opencomp` (`Microsoft.UI.Composition`), `openudk`
  (`Microsoft.Internal.FrameworkUdk`), `opencore` (`twinapi.appcore`'s
  `CoreApplication` surface), `opendispatch`, and `openxaml`. It is verified
  against the official binaries on Windows, not against a host's emulation.
- **Tools (C).** `xwinui` (cross-compile, package, deploy), `xbf` (XAML binary
  format), `comptrace` / `annotate.py` (ABI discovery), and a preview/render
  path that shows a XAML document without starting the real engine.

Wine is a **host**, not a target. The Wine patch series and its scripts have been
removed: they were measurement scaffolding for the Windows side, not a place where
features are added. Any finding that would need another Wine patch is
out of scope: it means the runtime is answering the wrong contract.

## The split

| piece | who provides it | why |
| --- | --- | --- |
| `Microsoft.ui.xaml.dll` (XAML engine, and the `Microsoft.UI.Content` classes) | official binary, later `openxaml` | it is in the App SDK package, not in Windows |
| `dcompi.dll` (`Microsoft.UI.Composition` server) | **us** (`opencomp`) | App SDK component |
| `dwmcorei.dll`, `DwmSceneI.dll`, `marshal.dll` (lifted compositor core and transport) | not replaced: our compositor takes their place | App SDK components; we answer the ABI they expose instead of rebuilding them |
| `Microsoft.Internal.FrameworkUdk.dll` (149 flat C exports) | **us** (`openudk`) | App SDK component; the XAML DLL imports 43 of them statically |
| `Microsoft.UI.dll`, `Microsoft.UI.Windowing.Core.dll` | official binaries | thin, and available in the package |
| `dcomp`, `dwmapi`, DWM, `win32k`, `user32`, `gdi32`, `combase` | the host platform | system services, never ours |
| `d3d11`, `dxgi`, `d3d12` | the host platform, or DXVK / vkd3d-proton | graphics API, not our layer |
| `d2d1`, `dwrite` | the host platform | rasterisation and text |
| .NET runtime | the application's own deployment | only needed for C# applications |

## Why the runtime is not wine-uwp

`wine-uwp` and `wine-ultimate` patch Wine so that packaged applications start:
they work on the platform side (win32k, the application model, system
DirectComposition) and let Microsoft's own XAML and CoreMessaging binaries run on
top. That is a different consumer and a different split.

The App SDK's composition is *lifted*: `dcompi` is a DLL in the application's own
package, `dwmcorei` exports `LiftedCompositionEngine_Initialize`, and the engine
runs in the application process. The object model we answer is
`Microsoft.UI.Composition`, not `IDCompositionVisual`.

Their work is still read: it is the best available description of how the
platform side behaves. The rule from the licensing section applies — read for
behaviour, do not translate.

## XAML engine references

The engine itself is not part of the App SDK, so its internals are not something
this project can copy. What is worth reading is how other projects structure a
XAML-compatible engine:

- **Uno Platform** (MIT): a WinUI-compatible API implemented over Skia and Web
  Assembly. Its XAML parser, dependency-property system, and layout contracts are
  the closest published match to what `openxaml` has to answer.
- **Avalonia** (MIT): its own XAML dialect with a self-drawn renderer; useful for
  the parser, styling, and layout pieces, not for WinUI ABI compatibility.
- **microsoft/microsoft-ui-xaml**: the XAML compiler and the WinUI controls are
  public; the engine is not. The compiler and the metadata are the contract we
  target, not an implementation reference.
- **WinMD metadata** (`Microsoft.UI.Xaml.winmd` and friends): the authoritative
  definition of every type, interface, and activation entry the engine exposes.

The rule is the same as for Wine: these are references for structure and
behaviour. Code is not copied; a cross-license copy would put a foreign license
into an MIT crate and usually imports assumptions that do not hold for an
ABI-compatible runtime.

## A worked boundary case

Microsoft's own `Windowing/cpp/cpp-win32` sample (`WindowsAppSDK-Samples`) is a
Win32 application that uses the App SDK:

```cpp
m_appWindow = winrt::AppWindow::Create();                  // App SDK, ours to answer
HWND hwnd = winrt::GetWindowFromWindowId(windowId);        // openudk's Windowing_*
auto interop = compositor.as<ABI::Windows::UI::Composition::Desktop::ICompositorDesktopInterop>();
interop->CreateDesktopWindowTarget(hwnd, true, &target);   // the SYSTEM compositor
```

The first two lines are ours. The third belongs to the host platform: the system
DirectComposition API that talks to DWM. That is why the *lifted* stack
(`Microsoft.UI.Composition` in the application's own package) is the one worth
replacing — replacing it is what makes the draw path independent of the system
compositor.

## Licensing

- `opencomp`, `opencomp-core`, `openudk`, `xwinui`, `comptrace`: MIT.
- DXVK (zlib) and the `windows`/`windows-*` crates (MIT/Apache-2.0) can be used
  directly.
- Wine, vkd3d-proton, Uno Platform, and Avalonia are read as descriptions of
  behaviour, never translated. Wine and vkd3d-proton are LGPL-2.1; Uno and
  Avalonia are MIT. In every case a translation would be a derivative work and
  would import assumptions that do not hold here.

## What "native" means here

Nothing runs natively. A WinUI 3 application is a Windows PE that calls the
platform's `user32`, `combase`, `d3d11`, `d2d1` and `dwrite`; replacing the App
SDK's DLLs does not remove any of that. The runtime's contract is the App SDK ABI,
not the host: the same DLLs must answer correctly on Windows, under Wine, or under
any future Windows-compatible loader. `xwinui` is the build half — cross-compile,
package, deploy — and runs nothing by itself.

## Engine language

The XAML engine is Rust. The reasons are the ABI (COM vtables and
`DllGetActivationFactory`), the per-frame path (no GC pauses), deployment (one dynamic
library, no CoreCLR inside every application), and consistency with `opencomp` and
`openudk`. C# is used only as a contrast: Uno Platform's parser output and Avalonia's
layout code are read to check behaviour, and a test client may be written in C#, but no
.NET runtime is embedded in the engine.

The first slice is vertical: one Gallery page through `xbf` -> object tree -> dependency
properties -> layout -> render commands, implementing only the controls and properties that
page uses.
