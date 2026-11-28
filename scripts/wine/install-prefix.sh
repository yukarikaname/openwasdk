#!/bin/sh
# Put the openwasdk runtime into a Wine prefix, the way setup_dxvk.sh does for DXVK.
#
# This is only the user-mode half. ALPC, the dcomp body, win32u and the CoreUICreate
# signature live in the Wine build, so an unpatched Wine gets the files and still cannot
# run anything: the script says which Wine it installed into and stops if it cannot find
# one. What it does copy is the DLL that replaces twinapi.appcore (the private
# CoreApplication interface), Wine's own copy under a second name so the replacement can
# forward to it, the WinRT class registrations that make activation find dcomp, and
# DXMT in the install tree where Wine loads its builtins from.
#
# usage: install-prefix.sh <wine-install> <prefix> [payload-dir]
#   wine-install  the tree holding bin/wine and lib/wine
#   prefix        WINEPREFIX to create or update
#   payload-dir   where opencore.dll is built (default: target/x86_64-pc-windows-gnu/release)
set -e

wine_dir=${1:?usage: install-prefix.sh <wine-install> <prefix> [payload-dir]}
prefix=${2:?usage: install-prefix.sh <wine-install> <prefix> [payload-dir]}
payload=${3:-$(cd "$(dirname "$0")/../.." && pwd)/target/x86_64-pc-windows-gnu/release}

wine=$wine_dir/bin/wine
builtins=$wine_dir/lib/wine/x86_64-windows
system32=$prefix/drive_c/windows/system32

[ -x "$wine" ] || { echo "no wine at $wine" >&2; exit 1; }
[ -f "$builtins/twinapi.appcore.dll" ] || { echo "no builtins at $builtins" >&2; exit 1; }
[ -f "$payload/opencore.dll" ] || { echo "no opencore.dll in $payload" >&2; exit 1; }

export WINEPREFIX=$prefix

# The prefix is created on first use, and wineboot only writes the DLL placeholders; the
# real files come from the install tree, which is why it has to exist before the copies.
# Mono and Gecko are turned off for the boot: wineboot hangs in their installer, which
# leaves a wineboot and a rundll32 behind forever instead of failing, and neither is needed
# by the App SDK.
[ -d "$system32" ] || WINEDLLOVERRIDES="mscoree,mshtml=" "$wine" wineboot -u >/dev/null 2>&1 || true
mkdir -p "$system32"

# Wine's twinapi.appcore keeps serving every class except CoreApplication, and the
# replacement forwards to it, so it has to survive under a name the loader will not
# resolve back to the replacement.
cp "$builtins/twinapi.appcore.dll" "$system32/twinapi.builtin.dll"
cp "$payload/opencore.dll" "$system32/twinapi.appcore.dll"

# Activation looks under HKLM\Software\Microsoft\WindowsRuntime\ActivatableClassId;
# dcomp and the replacement are invisible until named there.
"$(dirname "$0")/register-winrt.sh" "$wine" "$prefix"

"$wine" reg add 'HKCU\Software\Wine\DllOverrides' /v twinapi.appcore /t REG_SZ /d native /f >/dev/null
# A crashing probe otherwise leaves a winedbg --auto behind for every run.
"$wine" reg add 'HKCU\Software\Wine\WineDbg' /v Disabled /t REG_SZ /d yes /f >/dev/null

# DXMT has to sit in the install tree: its unixlib companion is a .so, and a prefix-level
# d3d11.dll alone cannot load it.
if [ -d "$payload/../dxmt" ]; then
    dxmt=$payload/../dxmt
    cp "$dxmt"/*.dll "$builtins"/ 2>/dev/null || true
    cp "$dxmt"/winemetal.so "$wine_dir/lib/wine/x86_64-unix/" 2>/dev/null || true
fi

# The boot leaves wineserver and the service processes behind; shut the prefix down so a
# fresh install does not look like a leak.
"$wine_dir/bin/wineserver" -k >/dev/null 2>&1 || true

echo "installed into $prefix"
echo "  twinapi.appcore.dll  $(wc -c < "$system32/twinapi.appcore.dll") bytes (opencore)"
echo "  twinapi.builtin.dll  $(wc -c < "$system32/twinapi.builtin.dll") bytes (wine)"
echo "run an app with:"
echo "  WINEPREFIX=$prefix WINEDLLOVERRIDES=twinapi.appcore=n $wine <app.exe>"
