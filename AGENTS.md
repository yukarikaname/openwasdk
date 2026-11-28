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
of downloading it.

## Where the measurements live

- `docs/winui-anatomy.md` — what the shipped binaries contain, measured.
- `docs/gate-b.md` — what the official XAML engine does under Wine, and the three
  hypotheses already ruled out (registration, our compositor, a missing import).
- `docs/opencomp-stubs.md` — every `E_NOTIMPL` in the compositor and why.

Start from those rather than re-deriving facts that cost a day each.

## Tools that generate code

- `comptrace generate` — WinRT interfaces and `_Impl` traits from metadata.
- `comptrace scaffold --class X` — the `#[implement(...)]` list and a stub body per
  method, for a class you are about to implement.
- `comptrace stubs <dll>` — one export stub per name in a PE, with `--check`.
- `xwinui activation --sdk <msix>` — the activation table, from the SDK's own manifest.

## The Gallery run

Goal: get the unmodified WinUI Gallery past `Application.LoadComponent` under Wine, or
prove which rule is the hard stop. It currently reaches `MainWindow..ctor` and
`InitializeComponent`, and faults in `Microsoft.ui.xaml+0x866a6`, a helper that compares
`GetCurrentThreadId()` with the dword at +0x50 of an object it was handed; the object is
null, because a `QueryInterface` for `{d3616837-3ed3-484e-ac26-cf5be382541e}` at
`+0x28b188` produced nothing.

Do:

- Measure before claiming. Run from `target/gallery/out` with `WINEPREFIX=\
  /Volumes/Lenovo/wine/prefix-winrt`, `WINEDLLOVERRIDES=twinapi.appcore=n`, and
  `OPENWASDK_SOFT_FAILFAST=1 OPENCOMP_TRACE=1 OPENCOMP_STUB_OK=1`; the stack comes from
  `OPENCOMP_BT=1` plus `OPENWASDK_FAILFAST=1`.
- Keep `scripts/wine/patches` equal to the working tree. Verify by applying the series to a
  clean wine-11.22 and diffing **every** source file the series should carry, added files
  included; a file that exists only in the working tree is a hole in the series.
- Use `scripts/xaml/annotate.py <dll> <rva>` to decode call sites: it names imports and
  GUIDs.
- Record negative results in `docs/` in the same session (the vtable-copying wrapper is
  one: a copied table still passes the wrapper as `this`).
- Commit each measured step, kernel style, no dates in docs, files under 500 lines.
- Kill `winedbg` and leftover `wineserver` after a crashing run.

Do not:

- Change Microsoft's DLLs, d3d11, d2d1 or win32k, and do not propose the Wine changes
  upstream; the tree is a laboratory.
- Answer a private interface with a look-alike object when the caller reads fields behind
  the pointer. Stubs are for learning which slot is called, and a stub handed out in that
  role carries the creating thread id at +0x50.
- Add `\#[allow]`, or hard-code an absolute path in a script (pass it as an argument).
- Run `make install` without `scripts/wine/refresh-dxmt.sh` afterwards.
- Set timers, schedules or automations without being asked.

Next step: after the `{d3616837}` query the caller invokes a method through the answered
interface at vtable offset 0x20 and passes an out pointer; a declaration that puts the
wrong method there leaves that pointer null. Confirm which method the offset names (classic
COM has three methods before WinRT's IInspectable ones) and give it a real answer.
