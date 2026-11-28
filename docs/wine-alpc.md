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

## Where this sits in the rules

This is a **laboratory**, not the delivery path. The acceptance rule is that a change is
ours only if it is still needed once our DLLs are swapped back for the official ones; a
Wine built here is not something a user would install, and nothing in it is proposed
upstream. What it buys is the answer to one question that cannot be answered otherwise:
with a real core underneath, which of our DLLs are still wrong, and what does the engine
do with a core it can talk to.

## What it does now (2026-11-28)

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

## Where the wall is now (2026-11-28, later)

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


## Metal instead of Vulkan (2026-11-28)

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

## Why the real CoreUI is not a small patch either (2026-11-28, last)

Wine's `CoreMessaging!CoreUICreate` now forwards to the SDK's `CoreMsgCreateSession`, and with
ALPC in place that call no longer aborts -- it blocks. What it waits for is the genuine
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
