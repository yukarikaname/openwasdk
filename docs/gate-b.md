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
| `Application.Start`, generated bindings | not yet run | callback runs (Wine 11, our dispatcher) |
| `StackPanel` with `Spacing = 4`, three 10 px children | not yet run | **38 px**, child 10 px |

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

## What it took

Four things had to be true before the engine would start, and three of them were our
bugs rather than Wine's:

1. **A dispatcher.** The SDK's own `Microsoft.UI.Dispatching` server aborts on load
   under Wine, so `opendispatch` answers that namespace and the activation manifest
   points at it. `CreateOnCurrentThread` then answers `S_OK`.
2. **An apartment.** `Application.Start` needs one; without `RoInitialize` it returns
   `CO_E_NOTINITIALIZED`.
3. **The process policy question.** XAML asks `AppPolicyGetWindowingModel`; Wine's
   `kernel32` forwards it to a `kernelbase` function that aborts on call. The probe
   rewrites the engine's import slot at load time (`shim.rs`) and answers
   `ClassicDesktop`, because we do not patch Wine.
4. **Correct vtables.** `IActivationFactory`, `IPanel` and `IVector<UIElement>` were
   declared with an `IUnknown` base, which puts the first method where **`GetIids`**
   sits. `GetIids` answers `S_OK` and writes an array, so every "the class activated"
   result was a successful call to the wrong function. The vtables now start with
   `IInspectable`, and collection access goes through the generated
   `windows_collections::IVector<UIElement>` rather than a hand-written IID.

`XamlCheckProcessRequirements` returns different values between runs (`0xfa9b2008`,
`0xfa9e2008`) and does not gate the start, so its return is not treated as an HRESULT.

## What a window needs beyond the measurement

Creating a `Window` is where the engine stops being a measuring device and becomes an
application, and it reaches further than `Start` does:

- `Microsoft.Windows.ApplicationModel.Resources.dll` loads as soon as a window is
  created, and it imports exactly one function from `bcp47mrm.dll`, which Wine does not
  have. `crates/bcp47` provides that one function beside the engine; a wrong rejection
  there would lose a locale, so the shim accepts any non-empty tag.
- The window path asks for `Microsoft.UI.Composition.Compositor` by name, so the
  composition classes in the activation manifest must point at a DLL that is actually
  in the directory: `opencomp.dll`, not only its dispatcher sibling.

## What the next step is

> Superseded by the section at the end of this file (2026-11-28): the dispatcher
> answers, and the start now stops in the CoreUI session.

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

## The window start under Wine, step by step (2026-11-28)

The container is gone: the probe now builds on the host for `x86_64-pc-windows-gnu` and
runs under a native Wine 11.21, so one run is a few seconds and the loop is measured in
minutes. With `OPENCOMP_TRACE=1` the probe prints every activation the engine makes and
every step it reaches.

What `Application.Start` does under Wine today, in the order it happens:

1. `RoInitialize(STA)`, then `Microsoft.UI.Dispatching.DispatcherQueueController`
   (our `opendispatch.dll`) on the calling thread.
2. `XamlCheckProcessRequirements`, `AppPolicyGetWindowingModel` (answered by
   `crates/xaml-probe`'s import shim, since Wine has it as a stub).
3. `Windows.Foundation.Metadata.ApiInformation`, `Windows.ApplicationModel.DesignMode`,
   `Windows.Foundation.PropertyValue`, `Windows.UI.ViewManagement.UISettings` -- all
   answered by Wine.
4. `Windows.ApplicationModel.Core.CoreApplication`, asked for
   `{17b0e613-942a-422d-904c-f90dc71a7dae}`. Wine's `twinapi.appcore.dll` serves the
   class and not that interface, and combase then returns `E_NOINTERFACE`, which the
   engine reports as the failure of the whole start.
5. `Microsoft.Internal.FrameworkUdk.dll` loads the system `coremessaging.dll` and calls
   `CoreUICreate`, which Wine carries as a stub (`@ stub CoreUICreate` in
   `dlls/coremessaging/coremessaging.spec`). A failure there is fatal: the engine passes
   the HRESULT to `RoFailFastWithErrorContext` and Wine raises `c0000602`.

Two shims answer those two walls.

`crates/opencore` serves `Windows.ApplicationModel.Core.CoreApplication` from our side.
The class is added to the activation manifest by `xwinui activation --add
Windows.ApplicationModel.Core.CoreApplication=opencore.dll`; every interface the factory
does not answer itself is forwarded to Wine's own `twinapi.appcore.dll`, so nothing about
the class changes except the one interface. That interface is undocumented: it is not in
the Windows SDK contract metadata, and `comptrace lookup` finds nothing. What the engine
calls first is its slot 11, the add half of `FinalUnhandledErrorDetected`; the handler is
kept and a token returned, and nothing ever fires it.

`crates/coremessaging` serves `CoreUICreate`. Wine's file cannot be replaced by name (the
builtin wins for a system DLL), so the probe takes over `GetProcAddress` in
`Microsoft.Internal.FrameworkUdk.dll` and answers that one name from our DLL; everything
else is forwarded. The object it returns answers the session interface
`{4b2edab9-129b-4733-be15-356853d55ace}`, whose vtable starts at `IUnknown`: slot 3 hands
back a registrar, slot 8 an item (`{fcd9d50d-0902-4a2d-ac94-b95c3316f6a0}`), slot 12 a
handler registration, which is a struct whose first field is its vtable, not an object
pointer. Every other slot refuses with `E_NOTIMPL` and names its number.

The start now stops at the registrar: the engine asks it for
`{37a94c05-1aa6-4cea-9049-083bba055cec}` and gets `E_NOINTERFACE`.

### Why this is the hard part

The chain above is the CoreUI conversation with the window system, and none of it is
documented. Two independent projects have walked the same path and published what they
saw: `zcomputerwiz/WineExplorer` (`docs/uwp/WINAPPSDK.md`) reports the same
`CoreUICreate` stop, the same session interface, the same slots 3/8/12, and the same
`{37a94c05}` question on the item; `Cencyte/Penguin-LGHUB` (`tools/wine/coreui_probe.c`)
names the session interface `IMessageSession` and slot 3 `GetMessageRegistrar`, and notes
that the real CoreMessaging gets past it only to need `NtAlpcConnectPortEx`, which no Wine
implements. WineExplorer's conclusion after their own implementation is that the chain
"costs one probe on Windows and one run" per layer, with no known depth.

There is no Windows machine in this worktree, but there is one in CI. `OPENCOMP_COREUI=proxy`
asks the system for the real session on Windows and hands the engine a proxy: every method
is forwarded to the real object with the same arguments and printed with the HRESULT, and
the two slots that hand back objects -- the registrar and the item -- are wrapped as well.
The Windows job uploads that trace as an artifact. That is the specification the Wine-side
session has to meet, which is the same measurement WineExplorer made by hand.

The other two ways out, neither of them taken:

- Find what makes `WindowsXamlManager` enter the `CoreWindowIntegration` route at all.
  The route exists for a window-system-hosting conversation; a probe that creates its
  window itself and drives its own dispatcher should not need it, and a version or
  feature check answered the other way would skip the whole chain.
- Continue the walk blind: answer `{37a94c05}`, log the slot the engine calls, answer the
  next question. Each layer would be a guess about semantics nobody documents.

The measurement is unchanged: it still has not run under Wine. Gate A's frame on Windows
remains the only end-to-end evidence that the client draws through our compositor.

### The App SDK's own messaging was tried, and stops at ALPC

`CoreMessagingXP.dll` from the same SDK is a complete CoreMessaging, server side included,
and `CoreMsgCreateSession(0, &session)` returns the object `CoreUICreate` is supposed to
hand back -- the same session interface and all. That would be Microsoft's own
implementation instead of ours, so the probe tries it under `OPENCOMP_COREUI=xp`.

It loads and runs. Three ntdll calls Wine only stubs are answered first:
`NtCreateWaitCompletionPacket` (with an event -- the closest waitable object user mode can
produce), `NtAssociateWaitCompletionPacket` and `NtCancelWaitCompletionPacket` (accepted,
never fired, because no packet object exists to fire). Session creation then proceeds and
stops here:

```
fixme:alpc:NtAlpcConnectPort ... L"\\BaseNamedObjects\\CoreMessagingRegistrar-324:..."
err:seh:NtRaiseException Unhandled exception code c0000602
```

The registrar conversation is ALPC by design, so this route needs an ALPC implementation
inside Wine and cannot be answered by a shim. It stays in the probe, one environment
variable away, for the day Wine has one.

## What the Windows run says about the CoreUI question (2026-11-28)

The proxy run on the Windows runner (artifact `coreui-trace`, job `Test (windows)`) reaches
the callback and prints the measurement's steps. It never asks the system for the CoreUI
session: `CoreUICreate` is answered by the probe and never called. Under Wine the same
engine build calls it, so the CoreUI conversation is a branch the engine takes only when
something before it did not go the Windows way -- not an unconditional part of
`Application.Start`.

One candidate was ruled out rather than assumed. `Microsoft.UI.Windowing.Core.dll` and
`Microsoft.UI.Input.dll` import `DllGetActivationFactory` from `CoreMessagingXP.dll` and ask
it for `Microsoft.UI.Dispatching.DispatcherQueueController` directly, bypassing the
activation manifest; our stand-in refused that class, and `coremessaging` now forwards it to
`opendispatch.dll`. The engine still calls `CoreUICreate` afterwards, so the branch is not
about that class.

The two traces agree up to the last activation before the branch
(`Windows.UI.ViewManagement.UISettings`, reached through `RoActivateInstance`, which Wine
carries as a semi-stub) and diverge there. The next step is to name the class that call
activates, on both sides, and to walk the UDK function that decides.
