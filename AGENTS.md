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
