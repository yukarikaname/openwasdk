# Gate B, measured

The first stage of the ladder that loads Microsoft's XAML engine. Everything here is
output of `scripts/gate-b.sh` (`xaml-probe`/`layout-probe` against the 2.7.0 packages),
on Wine and natively on Windows, and nothing here is inferred from source.

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
| `Application.Start` | not yet run | returns `S_OK` without running the callback |

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

## What the next step is

`Application.Start`, called through **generated bindings**. The activation table is
built (`xwinui activation`), so the remaining defect is in the probe: `Start` returns
immediately -- on Wine and on Windows -- instead of running its callback, which is what
a wrong hand-written vtable does. `crates/xaml-probe` already carries
`IApplicationStatics::Start` and `ApplicationInitializationCallback` from the
generator, so the measurement belongs there, not in `layout-probe`'s hand-written
slots, and `layout-probe` should be retired once it has moved.
