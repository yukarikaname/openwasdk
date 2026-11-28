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

`scripts/wine/build-macos.sh` configures Wine 11.22 with `--enable-archs=x86_64`: the unix
side stays arm64, the PE side is x86_64, which is the same shape as the prebuilt Wine this
project already uses, so the same x86_64 binaries run on both. Only the subsystems this
project touches are enabled; everything else (`x11`, `alsa`, `dbus`, `gnutls`-adjacent
network stacks) is configured out.

The source is the release tarball rather than a git checkout, because a checkout has no
`configure` and `autoconf` is not installed here.

## Where this sits in the rules

This is a **laboratory**, not the delivery path. The acceptance rule is that a change is
ours only if it is still needed once our DLLs are swapped back for the official ones; a
Wine built here is not something a user would install, and nothing in it is proposed
upstream. What it buys is the answer to one question that cannot be answered otherwise:
with a real core underneath, which of our DLLs are still wrong, and what does the engine
do with a core it can talk to.
