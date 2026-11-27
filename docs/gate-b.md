# Gate B, measured

The first stage of the ladder that loads Microsoft's XAML engine. Everything here is
output of `scripts/gate-b.sh` (`xaml-probe` against the 2.7.0 packages), on Wine and
natively on Windows, and nothing here is inferred from source.

## What loads

`Microsoft.ui.xaml.dll` imports 43 functions from `Microsoft.Internal.FrameworkUdk.dll`.
The shipped copy of that DLL also imports `Bcp47Langs.dll`, `profapi.dll` and two
`api-ms-win-appmodel-runtime-internal` API sets, none of which Wine has, so **the engine
cannot load under Wine with the SDK as shipped**:

```
err:module:import_dll Library Bcp47Langs.dll (which is needed by
  Microsoft.Internal.FrameworkUdk.dll) not found
xaml-probe: LoadLibraryW ... failed: GetLastError=0x0000007e
```

With `openudk.dll` copied over it, the DLL loads, all eight exports are present, and
`Microsoft.UI.Xaml.Application` and `Microsoft.UI.Xaml.Window` hand back their factories.
All 43 imported names exist in the stand-in, so the load is a matter of the stand-in
existing rather than of implementing any of them.

## What does not

| probe | Windows | Wine |
| --- | --- | --- |
| `LoadLibraryW Microsoft.ui.xaml.dll` | ok | ok, **only** with our FrameworkUdk stand-in |
| `XamlCheckProcessRequirements` | `0x00000000` | `0xfa9b2008` |
| `Application` / `Window` factories | ok | ok |
| `Controls.*` factories, raw | **segmentation fault** | crash |
| `Application.Start`, hand-written call | returns `S_OK`, no callback | returns `S_OK`, no callback |
| `Application.Start`, generated bindings | not yet run | **blocks in its message loop** |

Two conclusions follow, and both change what the next step is.

1. **Raw `DllGetActivationFactory` is not an entry point for controls.** Activating
   `Microsoft.UI.Xaml.Controls.*` before the engine has a core faults natively as well,
   so a probe that activates every class in a loop measures nothing past `Window`. The
   engine expects `Application.Start` to create the core on a dispatcher thread first.
2. **The core does not initialize under Wine.** `XamlCheckProcessRequirements` is
   `S_OK` on Windows and `0xfa9b2008` under Wine; the DLL carries the string
   `reason::failed_dxamlcore_init` next to that export, so the check is reporting that
   the core could not be created. `Application.Start` then returns `S_OK` and never
   calls back, which is consistent with a core that is not there.

The compositor is the first suspect, because `dcompi.dll` is what the official stack
creates the core against and Wine's DirectComposition is a stub. Replacing that file with
`opencomp.dll` did not change the result yet, which means the class is not reached the
way a rename suggests: `Microsoft.UI.Composition.Compositor` is activated by class name,
so it needs a registration entry pointing at the DLL, not a file name.

## Regenerating the probe's bindings

`crates/xaml-probe/src/xaml_bindings.rs` is generated from the App SDK winmds and
`Windows.winmd` with `--minimal`. The filter is the class list the export probe still
names, the statics interfaces it casts, and the members the layout case calls:

```sh
cargo run -p comptrace -- generate target/appsdk \
    --namespace 'Microsoft.UI.Content.ContentIsland,Microsoft.UI.Content.IContentIslandStatics::{},Microsoft.UI.Xaml.Application,Microsoft.UI.Xaml.Window,Microsoft.UI.Xaml.IWindowStatics::{},Microsoft.UI.Xaml.Controls.Button,Microsoft.UI.Xaml.Controls.IButtonStatics::{},Microsoft.UI.Xaml.Controls.Border,Microsoft.UI.Xaml.Controls.Frame,Microsoft.UI.Xaml.Controls.IFrameStatics::{},Microsoft.UI.Xaml.Controls.Grid,Microsoft.UI.Xaml.Controls.IGridStatics::{},Microsoft.UI.Xaml.Controls.StackPanel,Microsoft.UI.Xaml.Controls.IStackPanelStatics::{},Microsoft.UI.Xaml.Controls.TextBlock,Microsoft.UI.Xaml.Media.SolidColorBrush,Microsoft.UI.Xaml.UIElement::{},Microsoft.UI.Xaml.FrameworkElement::{},Microsoft.UI.Xaml.IApplicationFactory,Microsoft.UI.Xaml.Controls.IStackPanelFactory,Microsoft.UI.Xaml.Application::Start,Microsoft.UI.Xaml.UIElement::Measure,Microsoft.UI.Xaml.UIElement::Arrange,Microsoft.UI.Xaml.UIElement::DesiredSize,Microsoft.UI.Xaml.FrameworkElement::Height,Microsoft.UI.Xaml.Controls.StackPanel::Spacing,Microsoft.UI.Xaml.Media.SolidColorBrush::Color,Windows.Foundation.Size,Windows.Foundation.Rect' \
    --minimal --out crates/xaml-probe/src/xaml_bindings.rs
```

Each entry is there for one reason:

- `Microsoft.UI.Xaml.Application` and `Application::Start` start the engine;
  `IApplicationFactory` creates the application inside the callback.
- `Window`, `Controls.{Button,Border,Frame,Grid,StackPanel,TextBlock}` and
  `Media.SolidColorBrush` are the classes the export probe activates. The
  `I*Statics::{}` entries are name-only shells: the probe only casts them, and
  projecting their methods would pull in dependency closures the probe does not use.
- `UIElement` and `FrameworkElement` are name-only class wrappers; the members that
  matter are selected one by one: `UIElement::{Measure,Arrange,DesiredSize}`,
  `FrameworkElement::Height` and `Controls.StackPanel::Spacing`.
- `Controls.Border` is the activatable child; `Controls.IStackPanelFactory` is how the
  composable `StackPanel` is created.
- `Windows.Foundation.Size` and `Rect` are the `Measure`/`Arrange` arguments and the
  `DesiredSize` result.
- `Microsoft.UI.Content.ContentIsland` is still in the export probe's class list.
- `Microsoft.UI.Xaml.Media.SolidColorBrush::Color` is the one method the class probe
  actually calls.

Two accessors are deliberately outside the filter. `Panel.Children` returns a
`UIElementCollection`, whose `IVector<UIElement>` windows-bindgen projects through the
`windows-collections` crate, and the probe keeps its dependency list to `windows-core`
and `windows-sys`. `start.rs` declares those two interfaces from their metadata IIDs
instead; that is the only hand-written ABI left in the probe.

The metadata comes from the cached packages:

- `Microsoft.WindowsAppSDK.WinUI`: `metadata/Microsoft.UI.Xaml.winmd`
- `Microsoft.WindowsAppSDK.InteractiveExperiences`: `metadata/10.0.18362.0/Microsoft.UI.winmd`
  and `Microsoft.Foundation.winmd`
- `Microsoft.Windows.SDK.CPP`: `c/UnionMetadata/10.0.26100.0/Windows.winmd`

The App SDK set is unpacked by:

```sh
cargo run -p xwinui -- sdk --out target/appsdk --metadata
```

`Windows.winmd` lives in the CPP package's union metadata and is read from the cached
package directly:

```sh
unzip -p ~/.cache/xwinui/microsoft.windows.sdk.cpp.10.0.26100.4948.nupkg \
    'c/UnionMetadata/10.0.26100.0/Windows.winmd' > target/appsdk/Windows.winmd
```

`XWINUI_NUGET_BASE` moves the download to another flat-container host when nuget.org is
not the closest one.

`--minimal` keeps the interface and `_Impl` definitions and omits inherited forwarders;
explicitly filtered classes still get a wrapper. The probe calls through the interfaces
rather than through the generated class helpers, which is also what makes it work
without combase registration. windows-bindgen writes no leading comment, so the file
carries a hand-written header with the `#![allow(...)]` the metadata spelling needs.
Nothing else is post-processed: the one delegate box in this selection,
`ApplicationInitializationCallbackBox`, is declared and used inside
`Microsoft::UI::Xaml`, so no crate-qualified path is needed (unlike the
`TypedEventHandler` step in `README.md`).

## What the check is not about

Three hypotheses were tested and are dead, so nobody has to test them again.

- **Registration.** A manifest generated from the SDK's own `AppxManifest.xml`
  (1033 classes over 27 DLLs, `Microsoft.UI.Composition.*` pointed at `opencomp.dll`)
  changes nothing: the check still returns `0xfa9b2008`.
- **Our compositor.** Wine loads `DWrite`, `UIAutomationCore`, `d3d11`,
  `CoreMessagingXP`, `Microsoft.UI.dll`, `Microsoft.UI.Windowing.Core.dll` and the
  XAML DLL during the check. It does **not** load `dcompi.dll` or
  `Microsoft.UI.Composition.OSSupport.dll`, so the composition server is not involved
  yet.
- **A missing import.** `err+all` and `+dcomp` produce no error and no unimplemented
  call at all during the check, so nothing in Wine is refusing a symbol.

For the record, Wine's `dcomp` is a full stub -- `DCompositionCreateDevice`,
`...2` and `...3` all return `E_NOTIMPL` -- so the day the compositor is reached, it
will have to be ours rather than Wine's.

## Wine cannot run the SDK's dispatcher

`Application.Start` needs a dispatcher on the calling thread and does not create one:
`Microsoft.UI.Dispatching.DispatcherQueueController` is a class in the **App SDK**, and
the SDK's activation table points it at `CoreMessagingXP.dll`. That DLL exports 28 C
names (`CoreMsgCreateSession`, `CoreUICall*`, …) and **no** `DllGetActivationFactory`, so
the class is reached through combase, and loading the DLL to ask is what fails:

```
wine: Call from ... to unimplemented function ntdll.dll.NtAllocateReserveObject, aborting
```

Wine does not implement the reserve object that `CoreMessagingXP` allocates at load time,
so no version bump helps: the whole DLL is unusable there. That is also why
`Application.Start` blocks on Windows *and* under Wine in different ways -- on Windows it
waits for a queue the caller was supposed to install, and under Wine the queue cannot be
installed at all.

This is inside our scope rather than outside it: `Microsoft.UI.Dispatching` is an App SDK
namespace, so the answer is a replacement DLL for it, the same way `dcompi.dll` is
answered by `opencomp.dll`.

## What the next step is

The dispatcher is still what stands between the call and the callback: on Windows
`Application.Start` waits for a queue, and under Wine `CoreMessagingXP` cannot be
loaded at all. `opendispatch.dll` is the replacement in progress.

The measurement no longer waits on a second probe. The callback creates the
`Application` through `IApplicationFactory`, builds the vertical `StackPanel`
(`Spacing = 4`, three 10 px `Border` children), measures and arranges it through the
generated `IUIElement`, and prints the line `layout-probe` used to print:

```
{"case":"stackpanel-spacing4","width":...,"height":...,"child0_height":...}
```

The engine is expected to answer 38 for the height -- three borders plus two gaps -- but
the line prints what it answers. Nothing in this worktree has observed that line yet:
there is no Windows host here, and Wine still fails before the callback. The Windows CI
step is the first place it will be seen. `layout-probe` is deleted; its hand-written
slots were the source of the faults the generated bindings avoid.
