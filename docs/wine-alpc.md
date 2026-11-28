# The ALPC question

Gate B's window needs a CoreUI core. The core is what `CoreMessagingXP.dll` builds, and it
is built on ALPC. Wine does not have ALPC, so the file cannot load, the UDK falls back to
the system `CoreMessaging.dll`, `CoreUICreate` is a stub, and the engine gets a session
with nothing behind it.

That chain was assumed for a while. It is now measured: run `NtAlpcConnectPort(NULL)` in
the Wine this project uses and it answers `0xc0000002`, `STATUS_NOT_IMPLEMENTED`. The
export exists; the implementation does not. Any amount of shimming on our side ends at
that wall, because the core's whole job is to be a port.

## What exists

`designgears/wine-uwp` is a Wine fork whose target is packaged Store apps on Linux. Its
`docs/Findings-M2.md` is the part that matters here: it implements server-side ALPC,
validates it with probes (connect, accept, sync request/reply, datagram with transferred
handle, disconnect, wait-completion-packet, multi-object wait), and then runs the
**genuine `CoreMessagingRegistrar`** as a Wine service. Once the registrar accepted the
app, `RegisterActivatableApplication` answered `S_OK` and the XAML `FrameworkView` was
created.

So the missing layer is not unknown work. It is written, and it is small.

## What to take, and what not to

The fork carries 547 changed files against Wine 11.22 -- `dcomp`, `windowscodecs`, `heap`,
the appx machinery, its own `wineuwp.dll`. Almost none of that is ALPC, and all of it is
aimed at UWP (WinUI 2 + .NET Native) rather than at the Windows App SDK.

`scripts/wine/apply-alpc.sh` takes twelve files:

| file | what it carries |
| --- | --- |
| `server/alpc.c` | the port object: queues, connection acceptance, message ids, handle transfer |
| `server/protocol.def`, `request_handlers.h`, `request_trace.h` | the seven `alpc_*` requests that wire the client to it |
| `server/queue.c`, `server/token.c` | waitability, and the impersonation the registrar insists on |
| `dlls/ntdll/unix/alpc.c` | the client side |
| `dlls/ntdll/ntsyscalls.h`, `dlls/ntdll.spec`, `dlls/wow64/syscall.c` | the syscall numbers |
| `dlls/ntdll/sync.c` | `NtCreateWaitCompletionPacket` -- the function `CoreMessagingXP.dll` aborts on today |
| `include/winternl.h` | corrected `ALPC_MSGFLG_*` values; Wine had `RELEASE` wrong |

Both files exist upstream and both Makefiles already list them, so this is twelve file
replacements, not a patch queue.

## Building it

The first build configured `--enable-archs=x86_64` on the arm64 host: an arm64 unix side
and an x86_64 PE side. That is the Windows-on-ARM shape, and it does not run here -- the
prebuilt Wine this project uses is x86_64 on both sides, executing under Rosetta, and the
arm64 build died on start with no output at all. So the unix side has to be built for
x86_64 as well, which is a cross build on this machine: `--host=x86_64-apple-darwin
--build=aarch64-apple-darwin`, with `clang -arch x86_64` for the unix objects, the mingw
x86_64 compiler for the PE objects, and `--with-wine-tools` pointed at the arm64 tree
because the tools (`winebuild`, `widl`, `wmc`) have to run on the build machine.

Only the subsystems this project touches are enabled; everything else (`x11`, `alsa`,
`dbus`, the network stacks) is configured out.

The source is the release tarball rather than a git checkout, because a checkout has no
`configure` and `autoconf` is not installed here.

The changes themselves are a patch series against the 11.22 tag, in
`scripts/wine/patches/`, applied with `scripts/wine/apply-patches.sh`; five patches
cover ALPC, dcomp, win32u, the `CoreUICreate` signature and the fail-fast dump, and a
tree built from a clean tarball plus that series is byte-identical to the one used here.
After `make install`, `scripts/wine/refresh-dxmt.sh` puts DXMT back: the install step
copies Wine's own `d3d11`/`dxgi` over it and drops the engine to a feature level XAML
rejects.

## Where this sits in the rules

This is a **laboratory**, not the delivery path. The acceptance rule is that a change is
ours only if it is still needed once our DLLs are swapped back for the official ones; a
Wine built here is not something a user would install, and nothing in it is proposed
upstream. What it buys is the answer to one question that cannot be answered otherwise:
with a real core underneath, which of our DLLs are still wrong, and what does the engine
do with a core it can talk to.

## What it does now

The cross build completes (`Wine build complete.`) and the patch is live:

    NtAlpcCreatePort=... NtAlpcConnectPort=... NtAlpcSendWaitReceivePort=...
    NtAlpcConnectPort(NULL) = c000000d      STATUS_INVALID_PARAMETER
                                            (was c0000002, NOT_IMPLEMENTED)

`c000000d` rather than `c0000002` is the whole point: the port object now exists and
validates its arguments. On the same Wine, the Gate B probe reaches
`DispatcherQueueController.CreateOnCurrentThread hr=S_OK`, `Application.Start`, and then
the engine's own `UISettings` — all of which the Wine in use before this could not do.

Two things had to be fixed to get there, and neither is ALPC:

- `wineboot` hangs while installing Wine Mono (`control.exe appwiz.cpl install_mono`), so
  the prefix never finishes booting and every process waits on the boot event forever. The
  documented workaround applies: `WINEDLLOVERRIDES="mscoree,mshtml=" wineboot -u`.
- The build does not generate the WinRT class table (`HKLM\Software\Microsoft\WindowsRuntime\
  ActivatableClassId`), so combase cannot resolve `Windows.Foundation.Metadata.ApiInformation`
  and the engine fail-fasts at `RoGetActivationFactory`. The entries are produced by
  `widl --register` at build time and something in this cross build skips them; exporting
  the subtree from a working prefix and importing it gets past, which is enough to keep
  experimenting but is a build gap, not a fix.

The next wall is inside the engine rather than in front of it: the probe now answers a
`QueryInterface` for `{9d4b87ff-6dca-5ce2-b69ce52705b0fef4}` with the probe's own `E_FAIL`
and combase fail-fasts on it.

## Where the wall is now

With ALPC, wine-uwp's dcomp, the WinRT class registrations and a patched
`RtlGetDeviceFamilyInfoEnum` in place, the probe reaches XAML's own composition setup.
`dwmcorei.dll` -- the lifted DWM core -- still refuses to initialise (its `DllMain`
returns `FALSE`, `ERROR_DLL_INIT_FAILED`, even in a bare process). Forcing its
`DllMain` to answer `TRUE` moves the failure forward to graphics:

    wined3d_select_feature_level: None of the requested D3D feature levels is supported
    RoFailFastWithErrorContext 0x80004005

The build had no Vulkan, so wined3d fell back to macOS OpenGL, which tops out below what
XAML asks for. Getting Vulkan as far as a device took three things:

- `SONAME_LIBVULKAN` in `include/config.h` came out of configure as the whole
  `otool -L` identity line -- tab included -- so `dlopen` could never succeed;
- `libvulkan.dylib` and `libMoltenVK.dylib` borrowed from the prebuilt Wine;
- a MoltenVK ICD manifest in `VK_ICD_FILENAMES`; without it `vkCreateInstance` answers
  `VK_ERROR_INCOMPATIBLE_DRIVER` (-9).

After that `D3D11CreateDevice` succeeds on the Vulkan renderer, but wined3d still reports
no usable feature level, and the MoltenVK instance is created at Vulkan **1.0.323** while
the prebuilt Wine's own runs report **1.4.323**. That difference is the next thing to
look at: the feature-level table wined3d builds depends on the API version the instance
was created with.


## Metal instead of Vulkan

wined3d on this machine tops out below what XAML asks for whatever the renderer: the GL
backend cannot reach the feature levels, and the Vulkan backend needs MoltenVK wired up
exactly right. DXMT replaces `d3d11.dll`/`dxgi.dll` with a Metal implementation and
reports `D3D_FEATURE_LEVEL_11_1` immediately:

    info:  Maximum supported feature level: D3D_FEATURE_LEVEL_11_1
    info:  Using feature level D3D_FEATURE_LEVEL_11_1

Getting there needs DXMT built against *this* Wine, not the prebuilt release: the PE
side's unixlib has to be installed as a builtin, and a prefix keeps its own copy of
`d3d11.dll` which wins over anything else. So the recipe is an installed Wine
(`make install DESTDIR=...`) plus `meson install` of DXMT into that layout;
`scripts/wine/build-dxmt.sh` is the whole of it, LLVM 15 included.

With that, the probe reaches `measure window content` on Metal. The wall is now the one
place nothing on this side can patch around: `dwmcorei.dll`'s `DllMain` returns
`FALSE` -- relay shows it creating a D2D1 factory successfully and then unwinding -- so
the lifted DWM core never initialises, XAML's composition helper keeps a null device, and
the process faults on `[null + 0x360]`.

## What `CoreUICreate` actually takes

The forwarding wrapper used to declare `CoreUICreate(flags, session)` and pass both
through to `CoreMsgCreateSession`. The first Gallery run under that wrapper showed the
declaration was one parameter wrong:

    fixme:messaging:CoreUICreate flags ac1ce7b0 session 0000000000000000 -> 0x80004003

`0xac1ce7b0` is the low half of a stack address, not a flag word. Disassembling the
caller explains it: `Microsoft.Internal.FrameworkUdk.dll` resolves the export with
`GetProcAddress` and sets only `rcx` at the call site.

    18000dd0c  movq $0x0, 0x20(%rsp)     ; session = NULL
    18000dd15  lea  rcx, [rsp+0x20]      ; &session
    18000dd1a  call CoreUICreate

The SDK's own call site in `Microsoft.UI.Input.dll` supplies the other half of the
pairing: `CoreMsgCreateSession(0, &session)`. So the system entry point takes the out
pointer alone, and the session creator takes a flags word first. Forwarding the single
argument with flags zero gives what the SDK itself does, and the session is created:

    fixme:messaging:CoreUICreate session 000000CAAD78FDA0 -> 0

With a real session the Gallery moves one step further and fails on `0x80004001`,
`E_NOTIMPL`, from the same XAML frame. That is a different door, and the stack names it
before any guessing starts -- see the note on `RoFailFastWithErrorContextInternal2`
below.

## Naming the failure

`RoFailFastWithErrorContextInternal2` is where WinUI 3 ends an unrecoverable path, and
Wine's stub printed the code and nothing else, so every failure looked like whatever
logged last. Two things make it name itself:

A handler armed inside the replacement DLL is not enough: the process that raises the
fail fast is not the one that asked for the `CoreApplication` factory, so `OPENCOMP_BT`
never fired. The stub in Wine's `combase` always runs, and its dump prints the raising
thread's frames as `module+offset`:

    combase.dll+0x12fe2
    Microsoft.ui.xaml.dll+0x3d4fa9
    Microsoft.ui.xaml.dll+0x363af0
    Microsoft.ui.xaml.dll+0x16cf64
    Microsoft.ui.xaml.dll+0x151cad
    coreclr.dll+0x183b03

Two instructions of arithmetic turn that into the failing call: `Microsoft.ui.xaml.dll`
loads at its preferred base `0x180000000`, so `0x151cad` is the instruction after
`call 0x18016cc58`, and the frame above it is a branch the same function takes on a
failed `HRESULT`. That is how the `E_NOTIMPL` was placed without any symbols.

Symbols would still be better. The identity is in the image's CodeView record, and
`scripts/xaml/annotate.py <dll> --pdb` prints it with the URL it would be fetched from:

    Microsoft.ui.xaml.pdb
    6411D3D9B984BE17D2E95023B4EBD0022
    https://msdl.microsoft.com/download/symbols/Microsoft.ui.xaml.pdb/<key>/...

The servers do not have this build. The public symbol server answers 404 for the key, the
NuGet symbol host answers 403, and the image embeds no portable PDB -- its debug directory
carries CodeView, ILTCG and REPRO only. What remains is offset arithmetic:
`Microsoft.ui.xaml.dll` loads at its preferred base `0x180000000`, so an offset printed in
a stack frame is an address in the file, and the same tool names the imports and interface
identifiers around it.

## Two ways to answer the private interface

`CoreApplication` lives in Wine's `twinapi.appcore.dll`, and the engine asks its factory
for `{17b0e613-942a-422d-904c-f90dc71a7dae}` before anything appears. Two ways of
answering that query were tried.

The first was an application manifest naming our DLL as the class's server. The manifest
has to be embedded in the executable, which means `mt.exe`, which is an x86 program, so it
has to run under the prepackaged 32-bit Wine while the application runs under the 64-bit
one. Re-embedding the Gallery's manifest produced `0x800736b5`, an SxS assembly that could
not be found, and the application died earlier than before; the failure came from the merge
rather than from the entries, and chasing it further was not worth the detour.

The second puts the replacement where `RoGetActivationFactory` looks for the class:
`system32\twinapi.appcore.dll`. Wine's own file moves aside to `twinapi.builtin.dll`, and
the replacement forwards every interface it does not answer to it. The override
`twinapi.appcore=native` keeps the loader from resolving the name back to the builtin.
That is the arrangement that works -- the engine now registers
`FinalUnhandledErrorDetected` on the private interface and gets a session from the
composition side -- and `scripts/wine/install-prefix.sh` is all of it.

## The system half of the UDK

With a session created, the Gallery's next stop was `E_NOTIMPL` from
`Microsoft.Internal.FrameworkUdk.VisualDiagnosticsPort_Register`, which
`DXamlCore::RegisterVisualDiagnosticsPort` calls under `IFCFAILFAST`: the failure does
not propagate, it ends the process. Three pieces place the call:

- the import slot XAML uses resolves to `VisualDiagnosticsPort_Register`, and the dxaml
  source shows what it is for -- a CoreMessaging port Visual Studio attaches to;
- the UDK implements it through `Microsoft.Internal.FrameworkUdk.System.dll`, loaded with
  `LOAD_LIBRARY_SEARCH_SYSTEM32`. Wine answered `c0000135`, and the loader says so
  itself (`load_dll looking for ... in C:\windows\system32\`), so the file has to be
  in the prefix's system32, not beside the application;
- that DLL is a *loader* for a versioned function table. The UDK asks it for
  `GetFunctionTableId` and `GetVersionInformation`, validates each entry's size and
  kind, and installs its own stub for every entry the table does not provide. The stub
  behind the diagnostics slot returns `E_NOTIMPL`.

So what is missing is not one API but the OS-side function table: Windows 10 1809 and
later ship that DLL, Wine does not, and no amount of prefix surgery invents its contents.
Writing a table with the diagnostics entry is the next piece of work. Until then
`OPENWASDK_SOFT_FAILFAST=1` lets `RoFailFastWithErrorContextInternal2` return instead of
raising, which is enough to see what lies past the point -- the application then reaches
D2D text rendering, further than the fail fast ever allowed.

## What the Gallery needs past the session

With a session in hand the application moved through three more walls, each one a
different kind of problem:

- **The free-threaded marshaler refused `IAgileObject`.** `Microsoft.WinRT.Runtime`
  asks a wrapped object for the marker interface, and Wine's
  `ftmarshaler_inner_QueryInterface` answered `E_NOINTERFACE`, so the caller retried
  until a thread's stack ran out. Windows answers `S_OK` for the wrapper, which is
  agile by definition; with that fixed the retry loop disappears entirely.

- **The UDK's visual diagnostics port fails without the OS function table.** XAML
  registers the port under `IFCFAILFAST`, so a failure is fatal. The real
  `Microsoft.Internal.FrameworkUdk.dll` implements the port in a table at its own
  `.rdata`; the table's slot calls into machinery Wine does not have, and the
  `Microsoft.Internal.FrameworkUdk.System.dll` that Windows supplies is only a loader
  for a versioned function table (`GetFunctionTableId`, `GetVersionInformation`).
  A stand-in system DLL is loaded and its export resolves, but the UDK never asks it
  for the table in this build. Answering the port from our own UDK stand-in works --
  the application stops failing fast -- and breaks something else: with the stand-in
  in place, `Microsoft.UI.Xaml.Controls.Charts.dll` can no longer produce
  `XamlControlsChartsXamlMetaDataProvider`, because the rest of the stand-in is
  stubs. The shim replaces too much.

- **With the real UDK and `OPENWASDK_SOFT_FAILFAST=1`,** the application runs to the
  point where it draws text: `dwritefactory_CreateMonitorRenderingParams`,
  `d2d_device_create_device_context`, and then
  `d2d_device_context_DrawTextLayout ... hr 0x80004005`. It still creates no window of
  its own within the first minute, so text is the visible wall and window creation is
  the one behind it.

Two smaller gaps sat between those walls and the window:

- **`PropertyValue` answered `GetRuntimeClassName` with `E_NOTIMPL`.** A value from
  `PropertyValue.CreateDouble` is an `IReference<Double>`, and the projection asks for
  the class name to decide what the managed object is; without it the cast went to
  `IInspectable`, threw `InvalidCastException` ("Unable to cast object of type
  'WinRT.IInspectable' to type 'System.Double'") and ended the process while the theme
  resources were still loading. Giving property values the class name of the reference
  they implement moved the application all the way to `Microsoft.UI.Xaml.Window` for the
  first time.
- **conhost divided by a zero cell height.** This build has no font backend, so the font
  look-up fails and the cell size stays zero; every thread that opened a console crashed
  with a division by zero. A default text cell keeps the arithmetic defined.

Past the window the application asks for `dcompi.dll`, which is ours and was not deployed.
With it in the application directory the composition activation reaches `opencomp`, and
two more things became visible:

- **`bcp47langs.GetApplicationLanguages` was a stub.** The UDK calls it (with two small
  integers and an out pointer) and then `LanguageListAsMuiForm` with the value it got
  back, while resolving resource languages; the stub aborted the process through the
  unimplemented-function path. The list is opaque to the caller, so a token and the user's
  locale are enough to satisfy both calls.
- **The private statics slot wants one object that is two things.** XAML calls it with
  `{75f6468d-1b8e-447c-9bc6-75fea80b5b25}`, the `IDCompositionDevice2` IID, and then
  uses the result as a `Microsoft.UI.Composition` compositor: the same object has to
  answer both the dcomp device interfaces and the WinRT `ICompositor` family. Handing
  back only a dcomp device returns `S_OK` and crashes at the next dereference, and
  handing back only our compositor fails the same way from the other side.

So the next piece is a composite object in `opencomp`: a compositor whose
`QueryInterface` forwards the dcomp device IIDs to a device created through
`DCompositionCreateDevice3` and keeps the WinRT interfaces for itself.

Where it stands now: the application reaches `App.OnLaunched` and
`MainWindow.InitializeComponent`, and fails inside `Application.LoadComponent`. The
frames name `Microsoft.ui.xaml+0x866a6`, a helper that compares
`GetCurrentThreadId()` with a dword at +0x50 of the object it was handed; the object is
not one of ours, which means some earlier composition object was never built and the
caller is reading a null. The private statics answer carries two identities (a dcomp
device and a WinRT compositor), and the interfaces XAML asks for next are
`{5dda0f45-…}`, `{dabd9f1a-…}`, `{d4e43e6a-…}` on that object and `{d3616837-…}` on a
composition object the application later holds. A stub that answers a private interface
is not enough: the caller reads fields past the interface pointer, so the object has to
be the one the application expects, not a look-alike.

The order matters for what to do next: the diagnostics port is a debugging feature
whose failure only matters because of `IFCFAILFAST`, while D2D text is on the path
every WinUI 3 application takes.

## Why the real CoreUI is not a small patch either

Wine's `CoreMessaging!CoreUICreate` now forwards to the SDK's `CoreMsgCreateSession`. Called
with the argument list the lifted layer uses it returns a session rather than an error, and
the next call on that session is what asks for the rest. What that rest needs is the genuine
`CoreMessagingRegistrar` service, and wine-uwp's own notes list what hosting it needs beyond
ALPC: `WaitServiceState`, `QueryTransientObjectSecurityDescriptor`, the user32 ordinals
`NtUserInitThreadCoreMessagingIocp2`/`DrainThreadCoreMessagingCompletions2`, and combase's
`CoBeginProcessEvents`/`CoMsgWaitInProcessEvents`/`CoEndProcessEvents` pump, plus WinRT proxy
registration for cross-apartment marshalling.

Checking this tree with ripgrep: the user32 ordinals are here (they came in with wine-uwp's
win32u), `WaitServiceState` and `QueryTransientObjectSecurityDescriptor` exist as spec
entries, and `CoBeginProcessEvents` does not exist at all. So the registrar route means
adopting wine-uwp's combase work and its native-DLL override set as well -- which is most of
that fork, and that fork deliberately targets UWP/WinUI 2 and never touches the lifted DWM
core that WinUI 3 requires.

That is the boundary: the remaining Wine work is a tree adoption, not a patch, and even a
complete adoption does not address `dwmcorei`.
