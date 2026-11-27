# What a Windows App SDK app actually loads

Measurements taken from the 2.7.0 NuGet packages, x64 binaries only. Everything
below is read from the shipped files, not from documentation.

## The layers

```
app  ->  Microsoft.ui.xaml.dll            15,192,376   8 exports
             1,243 Microsoft.UI.Xaml interfaces
         dcompi.dll                        3,217,176   4 exports
             508 interface references, 248 source paths
         dwmcorei.dll                      3,426,088   2 exports
             146 source paths
         DwmSceneI.dll                     2,679,592   1 export
             62 source paths
         marshal.dll                         545,080  45 exports (LM_*)
```

The exports name the layering better than any call graph:

| DLL | Export | Meaning |
| --- | --- | --- |
| `Microsoft.ui.xaml.dll` | `DllGetActivationFactory` | WinRT server for the XAML classes |
| `dcompi.dll` | `DllGetActivationFactory` | WinRT server for `Microsoft.UI.Composition` |
| `dcompi.dll` | `WinUIDCompositionCreateDevice3` | creates the composition D3D device |
| `dwmcorei.dll` | `LiftedCompositionEngine_Initialize` | starts the compositor core |
| `DwmSceneI.dll` | `CreateDwmSceneRenderer` | creates the scene renderer |
| `marshal.dll` | `LM_CreateCompositionBuffer`, `LM_PresentCompositionBuffers` | shared-memory transport |

`dwmcorei.dll` ("i" for in-process) means the App SDK does not talk to the
system compositor over an RPC channel the way a UWP app does. The DWM core
ships inside the package and runs in the application process. That is what
"lifted" in `LiftedIXP` means, and it is the reason a replacement compositor is
feasible at all: the boundary is a DLL in our own package, not a service.

The source paths that survive in `.rdata` (assert and logging strings) name the
modules:

| DLL | Top directories |
| --- | --- |
| `dcompi.dll` | `dcomp/WinRTNested` (203 files), `WinRTNested/inproc` (16), `dwm-codegen/winrt/proxy/lib` (11), `dcomp` (10) |
| `dwmcorei.dll` | `dwmcore/resources` (50), `dwmcore/engine` (22), `dwmcore/Rendering` (21), `engine/inproc_lifted` (6), `resources/inproc_lifted` (3) |
| `DwmSceneI.dll` | `spectre/Engine/.../NativeRenderer` (31), `spectre/Engine/.../NativeRendererD3D11` (14), `spectre/DwmScene/Source` (10) |

`WinRTNested` is the WinRT projection written directly on top of dcomp;
`dwm-codegen/winrt/proxy` is the generated proxy for the same interfaces when
they are marshalled to the system compositor. Both live in `dcompi.dll`, so the
composition layer already contains a working example of "the same interface,
two backends" — the pattern `opencomp` needs.

## What XAML asks the compositor for

`Microsoft.ui.xaml.dll` contains 3,728 distinct interface identifiers. Found by
decoding every `lea r64, [rip+disp]` in `.text` that lands on 16 bytes in
`.rdata` with the GUID variant bits set and a version nibble of 4 or 5, then
resolving each against the metadata (`comptrace interfaces`) and against the
Win32 definitions in the `windows` crate.

| Class | Count |
| --- | --- |
| public WinRT, resolved | 1,491 |
| public Win32 COM, resolved | 71 |
| named by the Wine headers | 8 |
| unresolved | 2,158 |

The resolved WinRT ones, by namespace:

| Namespace | Interfaces |
| --- | --- |
| `Microsoft.UI.Xaml` | 1,243 |
| `Microsoft.UI.Composition` | 73 |
| `Microsoft.UI.Input` | 21 |
| `Windows.UI.ViewManagement` | 11 |
| `Microsoft.UI.Content` | 8 |

So the XAML engine's dependency on the layer we are replacing is 73 interfaces,
not 264. The most used are `ICompositionObject` (29 references), `IVisual`
(16), `ICompositionAnimation` (18), `ICompositionEasingFunction` (13),
`IContainerVisual` (6), `ICompositionBrush` (5), `ISpriteVisual` (4). A first
frame needs `ICompositionObject`, `IVisual`, `IContainerVisual`,
`ISpriteVisual`, `ICompositionBrush`, `ICompositionColorBrush`,
`ICompositionSurface` and the compositor itself; everything else can answer
`E_NOTIMPL` until something fails on it.

### What XAML activates by name

`Microsoft.ui.xaml.dll` carries exactly twelve UTF-16 strings that name a class in
`Microsoft.UI.Composition` or `Microsoft.UI.Content`, one occurrence each:

```
Microsoft.UI.Composition.Compositor
Microsoft.UI.Composition.CompositionObject
Microsoft.UI.Composition.CompositionEasingFunction
Microsoft.UI.Composition.CompositionPath
Microsoft.UI.Composition.CompositionCapabilities
Microsoft.UI.Composition.CompositionEffectSourceParameter
Microsoft.UI.Composition.SystemBackdrops.SystemBackdropConfiguration
Microsoft.UI.Content.ContentIsland
Microsoft.UI.Content.DesktopChildSiteBridge
Microsoft.UI.Content.DesktopPopupSiteBridge
Microsoft.UI.Content.CoreWindowSiteBridge
Microsoft.UI.Content.ContentExternalBackdropLink
```

No visual class is in the list — `ContainerVisual`, `SpriteVisual` and the
brushes are only ever created through the compositor instance — so a server that
answers `DllGetActivationFactory` for `Compositor` covers the composition half.
The `Content` half is where the window comes from, which matches the hosting
section of [`architecture.md`](architecture.md).

## The private boundary

Of the 2,158 unresolved identifiers, 14 also appear in `dcompi.dll`. Those are
the private interfaces that cross the XAML/composition boundary — the ones with
no metadata, which would have to be matched by hand. Attribution below is the
nearest source path in `dcompi.dll`'s `.rdata`:

| IID | Refs in XAML | Module |
| --- | --- | --- |
| `7bca64fd-150c-5d50-b56b-9f4f474c5930` | 6 | `dwm-codegen/winrt/proxy/lib/Composition.CompositionObject.cpp` |
| `b01bee51-063a-5fda-bd72-d76637bb8cb8` | 4 | `WinRTNested/WrtCompositionViewBox.cpp` |
| `c50898f6-c536-5f47-8583-8b2c2438a13b` | 4 | `WinRTNested/PropertyValueHelper.h` |
| `5f9c3d96-1e77-5980-8b28-7a9b8614a863` | 3 | `WinRTNested/inproc/WrtCompositionCapabilitiesi.cpp` |
| `61cf693f-db4c-579f-b905-5dd3d23cfd4d` | 3 | `dwm-codegen/.../Composition.CompositionCapabilities.h` |
| `1d4a1b0e-a493-58b9-80f4-200cd04400f9` | 2 | `dwm-codegen/.../Composition.CompositionObject.cpp` |
| `de522e05-2e72-52ee-bd1f-ac25a0708003` | 2 | `WinRTNested/WrtCompositionBatch.cpp` |
| `1def86ce-4be1-5ccc-ba28-05d746d9bb8d` | 2 | `dwm-codegen/.../Composition.CompositionObject.cpp` |
| `77fa9267-b867-5f77-8c02-7d85819d563b` | 1 | `dwm-codegen/.../Composition.CompositionObject.cpp` |
| `ecd63a61-4dbf-57bc-880c-a255dfe352c3` | 1 | `dwm-codegen/.../Composition.Private.SystemVisualProxyVisualPrivate.h` |
| `ffcdc0ea-b528-4bfa-a315-2f11f1c5e8f9` | 1 | `WinRTNested/WinRTExports.cpp` |
| `5dda0f45-a420-4cfd-844a-2cb7ac3d79c2` | 1 | `SceneNode::RuntimeClassInitialize` |
| `42614e61-b0aa-5e72-9354-2771db20b7a8` | 1 | `dwm-codegen/.../Composition.CompositionObject.cpp` |
| `c7e65ce2-fad5-5e3b-9c58-186ca8c1dd57` | 1 | `WinRTNested/inproc/WrtCompositori.cpp` |

None of these entries is large, which is the good news: the largest is six
references. Reading the code that uses each one splits the set in two:

- five are type descriptors, not interfaces. The code that loads them is a thunk
  of the form `lea rax, [guid]; mov [rdx], kind; mov [rdx+8], rax; ret`, a table
  entry for a boxed value, which is what `WinRTNested/PropertyValueHelper.h`
  suggests. `7bca64fd` is one of these.
- eight are `QueryInterface` targets: `mov rax, [rax]; call __guard_dispatch_icall`,
  slot 0 of a vtable. `61cf693f` has six such sites and no other use.

The distinction matters because a boxed-value IID is a platform type, not part of
the boundary being replaced. So the private composition surface is around nine
interfaces at most, several of which belong to the proxy that talks to the
system compositor and never appear in a composition-only program.

Two identifiers that looked private are not. `af86e2e0-b12d-4c6a-9c5a-d7aa65101e90`
is `IInspectable` and accounts for 232 references — that is ordinary `QI` volume,
not composition. `a4ed5c81-76c9-40bd-8be6-b1d90fb20ae7` is the IID of the
`AsyncActionCompletedHandler` delegate. Both are named in Wine's `include/*.idl`,
which is why that file is one of the naming sources: 4,101 IIDs come from it.

The remaining 2,144 unresolved identifiers are used only inside
`Microsoft.ui.xaml.dll`. Their neighbours include `IDWrite*`, `ID2D1*` and
XAML's own internal interfaces; they do not constrain `opencomp`.

## What each DLL needs to load

The import tables say which graphics stack each layer actually uses, which is not
obvious from the names:

| DLL | Imports | Graphics |
| --- | --- | --- |
| `Microsoft.ui.xaml.dll` | 33 | `DWrite.dll`, `d2d1.dll`, `d3d11.dll`, `UxTheme.dll`, `urlmon.dll` |
| `dcompi.dll` | 54 | `dcomp.dll`, `d2d1.dll`, `d3d11.dll`, `dxgi.dll`, `marshal.dll`, `wuceffectsi.dll` |
| `dwmcorei.dll` | 51 | `dcomp.dll`, `d2d1.dll`, `d3d11.dll`, `dxgi.dll`, `D3DCOMPILER_47.dll`, `WindowsCodecs.dll` |
| `DwmSceneI.dll` | 29 | `d2d1.dll`, `d3d11.dll`, `dxgi.dll` |
| `marshal.dll` | 11 | `Microsoft.UI.Composition.OSSupport.dll`, `ntdll.dll` |
| `Microsoft.UI.Xaml.Internal.dll` | 5 | nothing: CRT and `KERNEL32` |

Three consequences:

- XAML draws with D2D and DWrite directly, not only through composition surfaces.
  Text and shapes come out of XAML as pixels, which is why a replacement needs
  those libraries even if it never implements a surface brush.
- The renderer compiles its shaders at run time: `D3DCOMPILER_47.dll` is linked
  by the compositor core, so Wine needs that DLL as well as D3D11.
- `Microsoft.UI.Xaml.Internal.dll` is self-contained apart from the CRT, which
  fits what it turned out to be: a text layout engine.

## The flat C layer

`Microsoft.ui.xaml.dll` imports 33 DLLs. Two of them are not WinRT servers:

| DLL | Symbols imported by XAML | by `dcompi.dll` |
| --- | --- | --- |
| `Microsoft.Internal.FrameworkUdk.dll` | 43 | 10 |
| `Microsoft.UI.Windowing.Core.dll` | 1 (`ContentPreTranslateMessage`) | |

These are plain C exports, so the App SDK has a second, undocumented ABI next to
WinRT. `Microsoft.Internal.FrameworkUdk.dll` is 441 KB and exports 149 functions
in 40 groups; the ones XAML links are mostly optional features (input pane, text
prediction, MUI language propagation, media playback extensions, back button,
diagnostics) plus three that are structural:

- `Windowing_GetWindowIdFromWindow` and `Windowing_GetWindowFromWindowId`, the
  only `WindowId` ↔ `HWND` conversion. `include/Microsoft.UI.Interop.h` in the
  package implements the helpers by `LoadLibrary`-ing this DLL and
  `GetProcAddress`-ing those two names, so there is no WinRT path around them.
- `Containment_GetChangeEnabled`, the containment check XAML makes early.

`dcompi.dll`'s ten are `DxPrivates_*` (partner device, WARP extension) and
`DCompPrivates_CreateSharedVisual` / `CreateSharedVisualHandle` /
`OpenSharedResourceHandle` / `OpenAndAttachSharedTarget`, which is how the lifted
compositor attaches to a shared visual outside its process.

The import is static, not delay-loaded: `Microsoft.ui.xaml.dll` cannot be loaded
at all unless this DLL is found. Any project that wants to run the official XAML
binaries has to provide it, and that is a much smaller surface than the 214
methods of `Microsoft.UI.Content`: 6 `Windowing_*` exports carry the structure,
the rest degrade to a failed feature.

For completeness, the rest of the package's flat layer: `PowerNotifications_*`
(20), `ToastNotifications_*` (8), `PackageManagement_*` (9), `PushNotifications_*`
(7), `Media*Extension_*` (26), `Uup_*` (4), `SyncLayout_*` (2),
`DisplayPrivates_*` (5), `Theming_*` (2), `Text*` (7), `Input*` (12).

## Corrections to earlier assumptions

`Microsoft.UI.Xaml.Internal.dll` is not the private XAML ABI. It is 872 KB with
186 exports, and all but four of them are `Ls*` — the Line Services text layout
engine (`LsCreateParaBreakingSession`, `LsQueryEnumMathFraction`, ...). The XAML
engine itself is inside `Microsoft.ui.xaml.dll`, which exports only
`DllGetActivationFactory`.

There is no `Microsoft.UI.Composition.dll`. The composition implementation is
`dcompi.dll` plus `dwmcorei.dll` plus `DwmSceneI.dll`.

## Method

`comptrace iids` reproduces the identifier count:

```
comptrace iids Microsoft.ui.xaml.dll \
    --metadata Microsoft.UI.winmd --metadata Microsoft.UI.Xaml.winmd ... \
    --names wine-iids.txt
```

It is a byte scan for `lea r64, [rip+disp]` into `.rdata`, not a disassembler,
so it needs no capstone. Without the `windows` crate's Win32 names the count of
named identifiers is 1,554 rather than 1,491 + 71 + 8; the totals agree, which
is the check that matters. The remaining analysis was throwaway Python: `pefile`
for sections and `.pdata` function ranges, `capstone` to disassemble, and the
`windows` crate sources for the Win32 names. The identifier scan is
heuristic, not exact — a real IID that is loaded as a table entry rather than
by a `lea` is missed, and a 16-byte constant that happens to satisfy the bits
is counted. The numbers above are consistent with the parts that can be
checked (every `Microsoft.UI.Xaml` interface that XAML must create a class for
appears), but treat the unresolved set as an upper bound.
