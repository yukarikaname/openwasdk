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

Binding the object to its owner (a node whose clock comes from `compositor::clock_of`) is
in `easing.rs` and the gate still fails, so the owner was not the only hole. WER names
the exception this time:

    Exception Code    80004002   (E_NOINTERFACE)
    Fault Module      StackHash12_ce1
    Last Result       -1073741189  (0xC000027B, stowed)

A QI watcher over the object (`crates/opencomp/src/qi_trace.rs`, `OPENCOMP_QI_TRACE=1`)
now names what is asked:

    opencomp: easing created linear
    opencomp: object QI 8E1ECD0D-57D8-5BC9-9BCD-E43D0DD733C4

`8E1ECD0D` is `ICompositionEasingFunction`, which the object implements: that QI is
answered, and the crash comes after it with no further call into our object (the
`for_clock` trace, which runs when `Compositor()` needs to create one, never prints). So
the engine rejects the object's *identity* rather than missing an interface on it, and
the class stays with the engine.

The class's own hierarchy names two interfaces the object did not carry -- `IAnimationObject`
and `IClosable` -- and both are implemented now (`PopulatePropertyInfo` accepts, `Close`
does nothing). The gate still fails with the same HRESULT and still logs a single QI, so
the engine is not asking for them either: it dies after the QI it does make, which is one
we answer.

Measured, in order: activation reaches us, the statics build the object, exactly one QI is
asked (`ICompositionEasingFunction`, answered), and the stowed `E_NOINTERFACE` follows with
no further call into our code.

WinDbg on the crash dump names the caller:

    EXCEPTION_CODE: (HRESULT) 0x80004002 - No such interface supported
    SYMBOL_NAME:    Microsoft_UI_Xaml!OnFailure<4076>+3f
    FAILURE_BUCKET: STOWED_EXCEPTION_80004002_Microsoft.UI.Xaml.dll!OnFailure_4076_

so the QueryInterface that fails is XAML's own, made through `OnFailure<4076>`, not the input
host's. What it asks for is still unnamed -- the call is inside the engine and our watcher only
covers the objects we hand out. Next instrument: watch the statics object and the `Compositor`
we return from `Compositor()`, which is the other object XAML can reach from an easing
function. The next instrument is a vectored exception handler inside
`opencomp` that logs the stack of `0xC000027B` -- the same `trace_stack` machinery the
private statics already use -- because everything this side can see has been measured.

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
