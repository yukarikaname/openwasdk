# Gate C: the declarative C# path

Gate C is `microsoft/microsoft-ui-reactor`'s hello world: the smallest program that is
C# and takes no markup of its own. What it adds over Gate B is `.NET`: the App SDK
bootstrap, the runtime it loads, and a managed host that has to hand a window to native
code. What it deliberately does not add is XBF -- Reactor builds controls from factory
methods, so every failure here belongs to `.NET`, not to a compiler we do not have.

## The project

`gates/gate-c/` is one `.csproj` and one `.cs`.

- `WindowsPackageType=None` keeps the app unpackaged, so nothing has to arrange package
  identity before the process can start. Identity is a Gate D problem; here it would only
  blur the failure.
- `WindowsAppSDKSelfContained=true` copies the App SDK next to the executable. That is
  also the directory our DLLs go into when the same output runs under Wine, so both runs
  see the same layout.
- `Microsoft.WindowsAppSDK` is pinned to `2.7.0`, the version `crates/xwinui` extracts its
  runtime table from, so "our DLLs in front of the official ones" compares like with like.
  Reactor's own `.nuspec` asks for `2.2.x`; the upgrade is the same package family.

## Building it

Not on the host. `dotnet publish -r win-x64` gets all the way through C# compilation on
macOS and then dies in `Microsoft.WindowsAppSDK.SelfContained.targets`, which runs
`mt.exe` -- a Windows x86 program -- to merge the App SDK's SxS manifest into the
application's:

    /Users/.../microsoft.windows.sdk.buildtools/10.0.26100.4654/bin/10.0.26100.0/x86/mt.exe:
    cannot execute binary file

`EnableWindowsTargeting` is not the missing piece; a Windows tool is. So the build runs in
the baseline VM (`scripts/vm/`), which also means the ARM64 guest cross-publishes to
`win-x64` for the Wine side:

    ssh.py 'cd /d c:\work\gate-c & dotnet publish -c Release -r win-x64 -p:Platform=x64 -o c:\work\out'

`ssh.py` gives up after two minutes by default, which is shorter than a cold NuGet restore;
`SSH_TIMEOUT` raises it, and the build log goes to a file so a dropped session does not
take the error with it.

## Order of measurement

Same discipline as Gate B: run it on Windows first. A window there says the project is
sound and the runtime is where the build expects; only then does the same directory move
under Wine, where a failure means something the platform is missing rather than something
the project got wrong.

## What it does on Windows, and what it does under Wine

The Windows baseline is a window: `Gate C: 0`, a `Count` button, `VStack` spacing. Two
things were wrong before it appeared, and both are worth writing down because they cost a
debugging round each:

- The entry point needs `[STAThread]`, so the program is a `Program.Main` rather than
  top-level statements; top-level statements cannot carry the attribute, and WinUI creates
  WinRT classes before it shows anything.
- It has to run in the **interactive session**. Started from an SSH session, the process
  lands in session 0, where `DesktopWindowXamlSource::Initialize` ->
  `LiftedCompositionEngine_Initialize` closes an invalid handle and the process dies with
  `0xc000041d`. `schtasks /create /it` + `schtasks /run` puts it on the console session,
  which is also how the Gate B measurements were taken.

Under Wine the same directory fails in a straight line:

| build | failure |
| --- | --- |
| official self-contained runtime | `ntdll.NtCreateWaitCompletionPacket` is unimplemented; `CoreMessagingXP.dll` aborts on load |
| plus our `CoreMessagingXP`/`dcompi`/`opencomp`/`opencore`/`opendispatch` | `Windows.ApplicationModel.Core.CoreApplication` resolves to `opencore.dll`, then `coremessaging: no factory for Microsoft.UI.Dispatching.DispatcherQueue` |
| plus `coremessaging` answering every class through `opendispatch` | `opendispatch: timer 1 armed`, then XAML fail-fasts with `0x8007007e` right after `UISettings` |

That last position is the same one Gate B reaches, which is the expected result: Gate C is
B plus `.NET`, and `.NET` is not what fails. `win-x64` self-contained output runs the
CoreCLR, the MoltenVK-backed graphics stack and the App SDK bootstrap on this Wine; the
engine then stops at the CoreUI fallback, exactly as it does for the native probe.

## Two details worth keeping

- `cmd` does not wait for a `WinExe`, so `GateC.exe & echo %ERRORLEVEL%` reports `cmd`'s
  own status. A GUI program that "exits 0 with no window" is usually still unobserved.
- `mt.exe` runs under Wine (`~/.nuget/packages/microsoft.windows.sdk.buildtools/.../x86/mt.exe`),
  so the manifest embedded in a published `.exe` can be read and replaced without a Windows
  machine. Gate B's probe has an *external* `xaml-probe.exe.manifest`; a published app has
  an embedded one, and an external file would be ignored.
