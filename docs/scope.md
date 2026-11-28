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
| `dcomp.dll`, `dwmapi.dll`, DWM | Wine / Windows | the **system** compositor. Measured: the lifted stack *does* need it -- `dwmcorei`'s `DllMain` refuses to initialise without a DWM to connect to, and XAML then keeps a null device (`docs/wine-alpc.md`). This row is the one gap the Wine route has |
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

## Why not native

Three routes exist for running a WinUI 3 application outside Windows, and only one
of them is this project:

| route | what it runs | what it costs | who already does it |
| --- | --- | --- | --- |
| Wine + our App SDK layer | **unmodified** Windows binaries | the App SDK's own DLLs, nothing below them | nobody |
| a native framework, applications recompiled | applications written against it | a whole XAML runtime, control set, backend and per-language projection | Uno Platform (Apache-2.0) |
| a native Windows | everything, unmodified | win32k, user32, combase, d3d11, d2d1 | nobody, at Darling's scale |

What the measurement adds: the first route's promise of "unmodified Windows
binaries" holds for everything except the lifted compositor core. A WinUI 3 process needs a
DWM to connect to, Wine has none, and no open project implements one for Wine. Recompiling
does not change that -- the dependency is at run time. So Gate D (an unmodified Gallery) is
only reachable if somebody writes a Windows-compatible DWM for Wine, which is the
"third route" problem below and not part of this project's App SDK layer.

The second route is not a cheaper version of the first: it changes the subject
from "run this binary" to "build against us", and it duplicates a project that
already exists. The third is Wine's job.

What survives from the native idea is the **portable core**: the tree, the
flatten into a draw list and the animation sampler have no GPU and no OS in them.
That is why `opencomp-core` builds without wgpu, why the Windows backend is D3D11
(the device XAML hands over), and why a second backend stays possible without a
second set of semantics. It is not a promise to run applications natively.

## The compiler question

An open runtime should not need a closed compiler, and the way out is not to
write one. WinUI compiles markup because its runtime loads a compiled form; a
runtime we own can load XAML source instead — Microsoft's own `XamlReader.Load`
does exactly that today — or a precompiled form we define, in which case we own
both ends.

So the fork is which runtime we target, not which compiler we write:

| | runtime | markup input | Microsoft's compiler |
| --- | --- | --- | --- |
| now | the official `Microsoft.ui.xaml.dll` | the XBF *it* defines | needed, run under Wine |
| later | `openxaml` | XAML source, or our own format | not needed at all |

Writing an XBF *writer* now would be the expensive option that gets discarded the
day `openxaml` takes over: weeks of work to satisfy a contract defined by a
closed engine, thrown away when the engine goes. Reading XBF is the opposite
trade — MIT reference, a few thousand lines, and it stays useful, because every
existing application ships XBF.

Two things follow that are easy to get wrong:

- The compiler is a **build-time** tool on a developer's machine. A user running
  an application never invokes it, so it is not part of the runtime claim; the
  final milestone is "no Microsoft runtime DLL", not "no Wine anywhere". Wine is
  open source; the compiler is not, and it is invoked only while we still target
  Microsoft's engine.
- The same reasoning applies to packaging: `makemsix` is MIT and cross-platform,
  so there is nothing to replace, and unpackaged runs do not need it at all.

## The acceptance ladder

The goal is not "our tests pass", it is "an unmodified application runs". Each
gate is a program someone else wrote, run under Wine in CI, with our DLLs in
front of the official ones.

| gate | program | what it proves | state |
| --- | --- | --- | --- |
| A | our own composition-only client (`crates/opencomp-client`, asserted by `crates/opencomp/tests/gate_a.rs`) | the DLLs load, the ABI answers, the tree reaches a window | **done**: 16 ms to the first frame on Windows, 2.8 s under Wine, a frame counter asserted on both |
| B | a WinUI 3 app with no XAML markup: `Window`, `Grid`, one `TextBlock` built from code | XAML boots, D2D draws, Content hosts | **in progress**: under Wine 11 the engine loads, `Application.Start` reaches its callback on our dispatcher, and a `StackPanel` measures 38 px; a visible window is what remains (`docs/gate-b.md`) |
| C | `microsoft/microsoft-ui-reactor` hello world | the declarative C# path, no application XBF | B + .NET under Wine |
| D | `microsoft/WinUI-Gallery` | an unmodified application with templates, styles and navigation | C + XBF for the built-in themes |

Gate D is the milestone. Gate C is the first *foreign* application that is small
enough to debug. Gate B is where the work is: it needs a `Microsoft.UI.Dispatching`
replacement before the engine will start, because the one Microsoft ships aborts on
load under Wine.

Nothing in this ladder requires changing Wine, reimplementing D3D or D2D, or
porting wine-uwp.
