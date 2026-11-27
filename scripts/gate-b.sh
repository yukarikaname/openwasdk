#!/usr/bin/env bash
# Load the official Microsoft.ui.xaml.dll under Wine and report what it answers.
#
# Gate B starts here. The first run uses the DLLs as Microsoft ships them; the second
# puts our `openudk` in place of `Microsoft.Internal.FrameworkUdk.dll`, which is the
# layer XAML imports 43 functions from. Windows-only dependencies hang off the shipped
# one (`Bcp47Langs`, `profapi`, the appmodel API sets), so the shipped copy cannot even
# load under Wine -- that is what the stand-in is for.
#
#   scripts/gate-b.sh [sdk-dir]
#
# `OPENCOMP_SDK_FETCH=1` downloads the packages when the directory is empty.
# `XWINUI_NUGET_BASE` points that download at another flat-container host.

set -euo pipefail

sdk=${1:-target/appsdk}
run=${RUN_DIR:-target/gate-b}
wine=$(command -v wine64 || echo /usr/lib/wine/wine64)
target=target/x86_64-pc-windows-gnu/debug

if [ ! -f "$sdk/Microsoft.ui.xaml.dll" ] || [ "${OPENCOMP_SDK_FETCH:-0}" = 1 ]; then
	cargo run -q -p xwinui -- sdk --out "$sdk"
fi
echo "gate-b: $(find "$sdk" -maxdepth 1 -name '*.dll' | wc -l) SDK DLLs in $sdk"

# Run the probe once and keep its output. The run directory holds a copy of the SDK so
# a run can replace a DLL without touching the cached download.
probe() {
	local label=$1
	echo "gate-b: probing with $label DLLs"
	( cd "$run" && timeout -k 5 120 xvfb-run -a timeout -k 5 120 "$wine" ./xaml-probe.exe ) \
		2>&1 | tee "probe-$label.log"
	return "${PIPESTATUS[0]}"
}

rm -rf "$run"
mkdir -p "$run"
cp "$sdk"/*.dll "$run"/

export WINEPREFIX=${WINEPREFIX:-$PWD/target/wine}
export WINEDLLOVERRIDES=${WINEDLLOVERRIDES:-mscoree,mshtml=}
export RUST_BACKTRACE=1
# `err+loaddll` names the module that is missing when a load fails, which is the only
# failure mode that does not print an HRESULT.
export WINEDEBUG=${WINEDEBUG:-err+loaddll}
export XAML_DLL="$PWD/$run/Microsoft.ui.xaml.dll"
export OPENCOMP_XAML_PROBE_JSON=1

cargo build -q --target x86_64-pc-windows-gnu -p xaml-probe
xvfb-run -a "$wine" wineboot -u >/dev/null 2>&1 || true
# The probe and the DLLs sit together and the probe runs from that directory: a DLL
# loaded by full path does not put its own directory on the dependency search path.
cp "$target/xaml-probe.exe" "$run/"

official=0
probe official || official=$?

# Our stand-in for the flat C layer XAML imports. The exports are the 149 names the
# shipped DLL carries; the ones XAML actually imports are all present.
cargo build -q --target x86_64-pc-windows-gnu -p openudk
cp "$target/openudk.dll" "$run/Microsoft.Internal.FrameworkUdk.dll"
standin=0
probe standin || standin=$?

echo "gate-b: official exited $official, stand-in exited $standin"
# The stand-in run is the one Gate B needs; the official run is the baseline it is
# compared against.
exit "$standin"
