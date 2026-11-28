# Working notes for agents

## No network

The link to NuGet and crates.io from this machine is slow enough that a download
becomes the whole task. Everything needed is already on disk:

- `target/appsdk/` — the Windows App SDK 2.7.0 set: 30 DLLs (`Microsoft.ui.xaml.dll`,
  `dcompi.dll`, `Microsoft.Internal.FrameworkUdk.dll`, …) and 25 `.winmd`. Read-only.
- `~/.cache/xwinui/*.nupkg` — the packages `xwinui sdk` and `xwinui layout` cache.
- The cargo registry cache is populated: `cargo test --offline` works.

Do not run `xwinui layout`, `xwinui sdk`, `curl`, or a plain `cargo` command that
would update the index. If a package really is missing, say so in the report instead
of downloading it. The one exception is `msdl.microsoft.com`, which serves symbols and is
fast; fetching a PDB there is expected, see below.

## Where the measurements live

- `docs/winui-anatomy.md` — what the shipped binaries contain, measured.
- `docs/gate-b.md` — what the official XAML engine does under Wine, and the three
  hypotheses already ruled out (registration, our compositor, a missing import).
- `docs/opencomp-stubs.md` — every `E_NOTIMPL` in the compositor and why.
- `docs/composition-baseline.md` — measured: the engine's own `dcompi.dll` draws the Gallery
  window, our recording device dies in `LocalContentOutput::Initialize`, and why `dcompi` is
  a shim over the engine.

Start from those rather than re-deriving facts that cost a day each.

## Symbols before disassembly

Every shipped component keeps a CodeView record, so a fault offset has a name before it
has a guess. Do this **first**, before reading any disassembly:

```sh
python3 scripts/xaml/annotate.py <dll> --pdb        # prints the symbol-server URL
curl -sSL -o target/symbols/<name>.pdb <that url>
cp target/symbols/<name>.pdb "$(dirname <dll>)"     # llvm finds it by CodeView name
/opt/homebrew/opt/llvm/bin/llvm-symbolizer --obj=<dll> 0x180091178
```

`llvm-symbolizer` wants a virtual address (image base `0x180000000`), not an RVA, and
locates the PDB through the CodeView record, so the copy beside the DLL matters.
`llvm-pdbutil dump -publics <pdb>` lists what the symbols cover; Microsoft publishes
publics without line tables, so expect a function name and `??:0:0` for file and line.

Measured: `Microsoft.UI.Input+0x91178` is `LocalContentOutput::Initialize(ContentIsland *)`,
not the `CompositionHelper::...` a string search had suggested. The same route names
`dcompi.dll`, `dwmcorei.dll` and `Microsoft.ui.xaml.dll` offsets.

## Tools that generate code

- `comptrace generate` — WinRT interfaces and `_Impl` traits from metadata.
- `comptrace scaffold --class X` — the `#[implement(...)]` list and a stub body per
  method, for a class you are about to implement.
- `comptrace stubs <dll>` — one export stub per name in a PE, with `--check`.
- `xwinui activation --sdk <msix>` — the activation table, from the SDK's own manifest.

## The end-to-end case

WinUI Gallery is the acceptance case, not the goal. The goal is the App SDK ABI: the same
`Microsoft.UI.Composition`, `Microsoft.Internal.FrameworkUdk`, `CoreApplication` and XAML
activation surface answered by our DLLs on any host. Windows is where the ABI is verified
(the baseline VM); Wine is only a measurement host; its patch series and scripts have been removed.

Do:

- Check published work before writing a probe. `wine-uwp` / `wine-ultimate` describe how the
  platform side behaves; Uno Platform and Avalonia show how a XAML-compatible engine is
  structured; `microsoft/microsoft-ui-xaml` and the WinMD metadata are the contract.
- Resolve an address through its PDB before disassembling, and disassemble before guessing
  a vtable slot.
- Verify against Microsoft's binaries on Windows first. `scripts/vm` boots the baseline,
  `comptrace methods|classes|interfaces` reads the metadata, `annotate.py <dll> <rva>` decodes
  call sites.
- Record negative results in `docs/` in the same session.
- Commit each measured step, kernel style, no dates in docs, files under 500 lines.

Do not:

- Add Wine patches, or propose Wine changes upstream. A finding that needs another Wine
  patch means the runtime is answering the wrong contract.
- Copy or translate third-party engine code (Wine, Uno, Avalonia); read for behaviour only.
- Add `#[allow]`, or hard-code an absolute path in a script (pass it as an argument).
- Set timers, schedules or automations without being asked.
## Existing work to check before implementing anything Wine-side

Wine-side features implemented in this tree overlap with published forks. Check them before
writing a probe or guessing an ABI, and record what was found.

- **ZhymabekRoman/wine-ultimate** — Wine 11.10 plus the WinUI 3 / Windows App SDK /
  DirectComposition stack; branch `patches-archive` carries 153 patches with per-patch
  design and measurement notes. Already answers what this tree was rediscovering:
  `dcomp.spec` names ordinal 1040 `DCompositionCreateSharedVisualHandle`; its test shows
  that call returns a kernel handle of type "Composition" whose values step by four; the
  shared visual comes from the server request `dcomp_create_shared_visual` and is bound
  with `dcomp_set_shared_visual_info`; `dlls/frameworkudk.system/main.c` (223 lines)
  replaces the UDK; XAML activation is generated from the PE creator map (650
  `Microsoft.UI.Xaml.*` classes, 151 Controls classes).
- **designgears/wine-uwp** — about 49k lines over 196 files: own `dlls/dcomp`,
  composition swap chains, and `wineuwp` app-model host (package identity, activation,
  CoreWindow). Runs Shapr3D, Solitaire, Calculator.
- **geoffwoollams/wine-uwp** — Wine 11.22 plus UWP support, partial `Windows.UI.Xaml`
  (Minecraft, Halo Wars 2).
- **HITB 2023 "The Lost World of DirectComposition"** and Simon Mourier's
  direct-composition notes: background for the private DComp interfaces.

Next step: pick the runtime track first. For each App SDK component the baseline VM answers: which object, which interface, which slot, which out parameter. Start with `Microsoft.UI.Composition` core objects (Compositor, Visual, CompositionObject), write the ABI test against the official DLL on Windows, then make the Rust implementation pass the same test. In parallel, `xbf` plus `xwinui activation` should render a XAML document without the engine, so the XAML side can be checked without Wine or Windows.
## Reference checkouts

Read-only copies of projects used as references. They are never modified and never copied
into a crate; the licence rule in `docs/scope.md` applies.

| project | URL | local path | read it for |
| --- | --- | --- | --- |
| wine-ultimate | `github.com/ZhymabekRoman/wine-ultimate` | `../wine-ultimate` (full, 506M) | dcomp server protocol and the privates, `dlls/frameworkudk.system`, XAML activation generator, CoreMessaging build-number knob |
| wine-uwp (designgears) | `github.com/designgears/wine-uwp` | not cloned | own `dlls/dcomp`, composition swap chains, `wineuwp` app-model host |
| wine-uwp (geoffwoollams) | `github.com/geoffwoollams/wine-uwp` | not cloned | UWP `CoreWindow`, input, package identity |
| Avalonia | `github.com/AvaloniaUI/Avalonia` | `refs/Avalonia` (sparse) | XAML parser, styling, layout |
| Uno Platform | `github.com/unoplatform/uno` | `refs/uno` (sparse: `src/Uno.UI`) | WinUI-compatible API, dependency properties, parser |
| microsoft-ui-xaml | `github.com/microsoft/microsoft-ui-xaml` | `../microsoft-ui-xaml` (partial: `dxaml`, `src`) and `refs/microsoft-ui-xaml` (sparse: `dxaml`, `controls`) | XAML engine (`dxaml`), XAML compiler |
| LiberNT (ReactOS fork) | `github.com/LiberNT/LiberNT` | `refs/LiberNT` (sparse: `base/system/dwm`) | the host layer (PE loading, ntdll/win32ss) **and a working DWM**: `base/system/dwm` (`dwm.c` 5.4k lines, `gpucomp.c` 2.9k, D3D/GL backends, `NtDCompositionCreateConnection`) plus `dll/directx/wine/dcomp` for the client side; no XAML |
| ReactOS | `github.com/reactos/reactos` | not cloned | the upstream of the same host-layer material |
| Darling | `github.com/darlinghq/darling` | not cloned | architecture analogy for a host compatibility layer, not code |
| OpenSwiftUI | `github.com/OpenSwiftUIProject/OpenSwiftUI` | not cloned | architecture analogy for a framework reimplementation, not code |

These checkouts live outside the repository or under `refs/`, which is gitignored; keep them
out of commits. To recreate a sparse one:

```sh
git clone --depth 1 --filter=blob:none --sparse <url> refs/<name>
cd refs/<name> && git sparse-checkout set <paths>
```

Adding a path later only fetches that path: `git sparse-checkout add controls` in the
`microsoft-ui-xaml` checkout, for example.
