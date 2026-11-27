# What this project owns

OpenWASDK replaces the App SDK's own DLLs — the ones that ship inside the
`Microsoft.WindowsAppSDK.*` NuGet packages and land in
`runtimes-framework/win-x64/native/`. Nothing else. Everything a Windows
installation or Wine already provides is used as it is.

The test for a change: **if replacing our DLL with the official one on a real
Windows machine would make the change unnecessary, it is ours. Otherwise it
belongs to Wine, and we do not write it.**

## The split

| piece | who provides it | why |
| --- | --- | --- |
| `Microsoft.ui.xaml.dll` (XAML engine, and the `Microsoft.UI.Content` classes) | official binary, later `openxaml` | it is in the App SDK package, not in Windows |
| `dcompi.dll` (`Microsoft.UI.Composition` server) | **us** (`opencomp`) | App SDK component |
| `dwmcorei.dll`, `DwmSceneI.dll`, `marshal.dll` (lifted compositor core and transport) | not replaced: our compositor takes their place | App SDK components; we answer the ABI they expose instead of rebuilding them |
| `Microsoft.Internal.FrameworkUdk.dll` (149 flat C exports) | **us** (`openudk`) | App SDK component; the XAML DLL imports 43 of them statically |
| `Microsoft.UI.dll`, `Microsoft.UI.Windowing.Core.dll` | official binaries | thin, and available in the package |
| `dcomp.dll`, `dwmapi.dll`, DWM | Wine / Windows | the **system** compositor; the lifted stack does not need it |
| `d3d11`, `dxgi`, `d3d12` | Wine's wined3d, or DXVK (zlib) / vkd3d-proton (LGPL, linked) | graphics API, not our layer |
| `d2d1`, `dwrite` | Wine | rasterisation and text; XAML calls them directly |
| `combase` (`RoGetActivationFactory`, HSTRING), `win32k`, `user32`, `gdi32`, input stack | Wine / Windows | platform |
| .NET runtime | Wine / Microsoft | only needed for C# applications |

## Why this is not wine-uwp

`wine-uwp` patches Wine so that UWP applications run: it works on the platform
side (win32k, the UWP application model) and carries a 26k-line implementation of
the **system** DirectComposition. That is a different target with a different
consumer.

The App SDK's composition is *lifted*: `dcompi` is a DLL in the application's own
package, `dwmcorei` exports `LiftedCompositionEngine_Initialize`, and the engine
runs in the application process. There is no system compositor in the path, so no
part of Wine's `dcomp` is involved, and we do not need to touch Wine at all.

The two projects meet at one place: DirectComposition's *semantics* (visual trees,
transforms, clips, surfaces). Even there the code is different, because the
object model we must answer is `Microsoft.UI.Composition`, not `IDCompositionVisual`.

## A worked boundary case

Microsoft's own `Windowing/cpp/cpp-win32` sample (`WindowsAppSDK-Samples`) is a
Win32 application that uses the App SDK. Reading it shows where the line falls:

```cpp
m_appWindow = winrt::AppWindow::Create();                  // App SDK, ours to answer
HWND hwnd = winrt::GetWindowFromWindowId(windowId);        // openudk's Windowing_*
auto interop = compositor.as<ABI::Windows::UI::Composition::Desktop::ICompositorDesktopInterop>();
interop->CreateDesktopWindowTarget(hwnd, true, &target);   // the SYSTEM compositor
```

The first two lines are ours. The third is `Windows.UI.Composition.Desktop`, the
system DirectComposition API that talks to DWM — Wine has to provide that, and the
`dcomp` stub means it cannot yet. The sample is therefore not a target, and
more importantly it shows why the *lifted* stack (`Microsoft.UI.Composition` in
the application's own package) is the one worth replacing: it needs no DWM.

## Licensing

- `opencomp`, `opencomp-core`, `openudk`, `comptrace`: MIT.
- DXVK (zlib) and the `windows`/`windows-*` crates (MIT/Apache-2.0) can be used
  directly.
- Wine and vkd3d-proton are LGPL-2.1. They are called as separate DLLs, which is
  fine. Their source is not copied.
- `wine-uwp`'s `dlls/dcomp/composition.c` is read as a description of behaviour,
  never translated. A translation would be a derivative work and would put an
  LGPL file in an MIT crate; that trade is not worth it.

## What "native" means here

Nothing runs natively. A WinUI 3 application is a Windows PE that calls
`user32`, `combase`, `d3d11`, `d2d1` and `dwrite`; replacing the App SDK's DLLs
does not remove any of that, so the process is a Windows process under Wine (or
on Windows). `xwinui` is the build half — cross-compile, package, deploy — and
runs nothing by itself.

What does run natively is our own code: `opencomp-core` draws a tree with wgpu on
Metal, Vulkan or D3D12, and the two examples use that for previews and tests.
That is a way to look at a tree without a Windows binary, not a way to run an app.

An arm64 path exists and still involves Wine: native arm64 Wine runs arm64 PE,
and the App SDK package ships `win-arm64` and `win-arm64ec` binaries, so a chain
with no Rosetta is possible if our DLLs are built for `aarch64` too. On macOS the
x64 binaries need Rosetta, which is the only reason it came up.

## The acceptance ladder

The goal is not "our tests pass", it is "an unmodified application runs". Each
gate is a program someone else wrote, run under Wine in CI, with our DLLs in
front of the official ones.

| gate | program | what it proves | needs |
| --- | --- | --- | --- |
| A | our own composition-only client (`crates/opencomp-client`) | the DLLs load, the ABI answers, the tree reaches a window | `opencomp`, `openudk`, a swapchain |
| B | a WinUI 3 app with no XAML markup: `Window`, `Grid`, one `TextBlock` built from code | XAML boots, D2D draws, Content hosts | A + surface interop, dispatcher |
| C | `microsoft/microsoft-ui-reactor` hello world | the declarative C# path, no application XBF | B + .NET under Wine |
| D | `microsoft/WinUI-Gallery` | an unmodified application with templates, styles and navigation | C + XBF for the built-in themes |

Gate D is the milestone. Gate C is the first *foreign* application that is small
enough to debug. Gate B is where the work is.

Nothing in this ladder requires changing Wine, reimplementing D3D or D2D, or
porting wine-uwp.
