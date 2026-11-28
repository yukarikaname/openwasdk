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
