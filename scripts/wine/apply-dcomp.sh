#!/bin/sh
# Replace Wine's DirectComposition device with wine-uwp's implementation.
#
# Upstream `dlls/dcomp` is one file: a `DCompositionCreateDevice3` that answers
# `E_NOTIMPL`. Anything that composes through DirectComposition -- which is every WinUI
# frame -- therefore has no device at all. wine-uwp carries a real one: a new
# `composition.c` (the whole surface/visual/target model) and a rewritten `device.c`.
#
# One thing is dropped on the way in. Their `composition.c` calls
# `NtUserCallHwnd( hwnd, NtUserCallHwnd_FlushWindowSurface )`, a window-surface syscall
# added in their `dlls/win32u`; pulling that in means pulling in the display driver layer.
# Without the call the surface is presented on the next paint instead of eagerly, which is
# what a probe that only wants a first frame can live with.
#
# usage: apply-dcomp.sh <wine-uwp-checkout> <wine-11.22-source>
set -e

uwp=${1:?usage: apply-dcomp.sh <wine-uwp-checkout> <wine-11.22-source>}
wine=${2:?usage: apply-dcomp.sh <wine-uwp-checkout> <wine-11.22-source>}

for file in device.c composition.c dcomp.spec Makefile.in; do
	cp "$uwp/wine/dlls/dcomp/$file" "$wine/dlls/dcomp/$file"
	echo "applied dlls/dcomp/$file"
done

python3 - "$wine/dlls/dcomp/composition.c" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_text()
if "NtUserCallHwnd_FlushWindowSurface" not in text:
    raise SystemExit("expected the flush syscall in composition.c")
text = text.replace(
    "            NtUserCallHwnd( hwnd, NtUserCallHwnd_FlushWindowSurface );",
    "            /* flush is a win32u syscall this tree does not carry; see apply-dcomp.sh */",
)
text = text.replace(
    "    NtUserCallHwnd( hwnd, NtUserCallHwnd_FlushWindowSurface );",
    "    /* see apply-dcomp.sh */",
)
path.write_text(text)
print("dropped the window-surface flush")
PY
