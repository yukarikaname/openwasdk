# The composition baseline on Windows

What the baseline VM answered about the compositor, in the order it answered it.

## The engine runs the Gallery with the engine's own shim

`c:\work\gal-out` holds the framework package's layout (30 DLLs, including
`dwmcorei.dll`, `DwmSceneI.dll`, `Microsoft.ui.xaml.dll`, `Microsoft.UI.Input.dll`)
and the Gallery's published output. With the **official** `dcompi.dll` in place:

    WinUIGallery.exe  Id 3556  SessionId 1  Responding True
    MainWindowHandle  1442642
    MainWindowTitle   WinUI Gallery
    schtasks galrun   Last Result 267009   (0x41301, still running)

The screenshot of session 1 shows `WinUI Gallery - Search controls and samples` with its
content drawn. So the lifted compositor in this VM is healthy: `dcompi.dll` forwards to
`dwmcorei.dll`, and the failure below is ours, not the environment's.

## Our shim in front of the engine renders it

`c:\work\gal-out` with our `dcompi.dll` (1,230,386 bytes) and the engine's shim renamed to
`dcompi-ms.dll` (3,217,176 bytes):

    dcompi: engine device for 0x7ffb344d9820 -> 0x0

    modules: dcompi.dll  dcompi-ms.dll  dwmcorei.dll  Microsoft.UI.Input.dll  Microsoft.UI.Xaml.dll
    window:  hwnd=328550 vis=True class=WinUIDesktopWin32WindowClass title=WinUI Gallery

The window is drawn: the screenshot of session 1 shows the Gallery sidebar, hero and cards.
`Get-Process ... MainWindowHandle` can stay `0` for a Win32 window whose handle the process
table has not refreshed, so window state is read with `EnumWindows` from session 1, not with
that property.

## Our recording device fails inside the input host, not in the compositor

With `opencomp`'s recording device answering `IDCompositionDevice2`:

    WER: Microsoft.UI.Input.dll, exception 0xc0000409, fault offset 0x91178
    symbols: LocalContentOutput::Initialize(ContentIsland *)

`LocalContentOutput::Initialize` asks the object XAML gave it for a private interface
(`{bbcb00be-cd1...}` then `{c0eeab6c-...}`, `{c70dbce1-...}`) and then calls virtual slot
22 as `CreateTargetForHwnd`. A recording object that answers every QI and every slot does
not satisfy it, because the engine is not looking for a stand-in: it is looking for the
real lifted compositor object, which `dcompi.dll` + `dwmcorei.dll` construct.

## Milestone 2, measured on the first class

`OPENCOMP_MINE=CompositionEasingFunction` makes `dcompi` ask us for the class, and it
does:

    dcompi: ours: "Microsoft.UI.Composition.CompositionEasingFunction"

The page then dies before a window exists:

    WinUIGallery.exe  Last Result -1073741189  (0xC000027B, STATUS_STOWED_EXCEPTION)
    processes: 0        log: one line, the activation above

With the same tree and `OPENCOMP_MINE` unset the window comes back (pid 8268, dcompi.dll
and dcompi-ms.dll beside dwmcorei.dll), so the class is the cause and the environment is
fine. The reason is in our code, not in the engine: `CreateCubicBezierEasingFunction`
takes the owner compositor and ignores it (`crates/opencomp/src/easing.rs`), and the
objects are declared `composition_object!(X_Impl)` without a node, so `Compositor()`
answers with the process-wide instance instead of the one the caller handed to the static.
An easing object that claims the wrong compositor is a stowed exception waiting for the
first animation.

The class stays with the engine until the object is bound to that owner: bind the clock
through `compositor::clock_of(&owner)`, give the object a node, and rerun this gate.

## The rule this gives

Composition is the engine's, not ours. `dcompi.dll` here is a shim: it asks `dcompi-ms.dll`
(the untouched engine binary, shipped beside it) for every activation, and takes a class
over only when `OPENCOMP_MINE` names it. `WinUIDCompositionCreateDevice3` returns the
engine's device unless `OPENCOMP_RECORDING` asks for the recording one. That turns every
replacement into an A/B against a window that is known to appear, and keeps the private
interface question ("what does the engine expect here") in the measurement, not in a guess.

## Reproducing

    scripts/vm/push.py target/x86_64-pc-windows-gnu/release/dcompi.dll 'c:\\work\\gal-out\\dcompi.dll'
    SSH_TIMEOUT=60 scripts/vm/ssh.py 'schtasks /run /tn galrun'
    # then: Get-Process WinUIGallery | Select Id,MainWindowTitle

Symbols for any offset in those DLLs come from `annotate.py <dll> --pdb`, which prints the
symbol-server URL; download the PDB, copy it beside the DLL (llvm finds it through the
CodeView record) and run `llvm-symbolizer --obj=<dll> 0x180091178` with a virtual address.
Resolve the address before reading any disassembly.
