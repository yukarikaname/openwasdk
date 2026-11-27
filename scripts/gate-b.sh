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
# WineHQ installs its runners under /opt and replaces the distribution package, so
# neither a PATH lookup nor the old path is reliable.
wine=
for candidate in wine64 wine /opt/wine-stable/bin/wine64 /opt/wine-stable/bin/wine /usr/lib/wine/wine64; do
	if resolved=$(command -v "$candidate" 2>/dev/null); then wine=$resolved; break; fi
done
[ -n "$wine" ] || { echo "gate-b: no wine found" >&2; exit 2; }
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

# The probe reaches the App SDK classes by name, so a manifest has to point at the DLL
# that serves each one. Without the runtime package there is no manifest; the engine
# then fails at the first activation, which the report shows.
msix=${ACTIVATION_MSIX:-}
if [ -z "$msix" ]; then
	# `find` rather than `ls`: a missing file must leave this empty, not abort the run
	# under `set -e`.
	msix=$(find "$sdk/.." -maxdepth 1 -name 'Microsoft.WindowsAppRuntime*.msix' 2>/dev/null | head -1)
fi
if [ -n "$msix" ] && [ -f "$msix" ]; then
	cargo run -q -p xwinui -- activation --sdk "$msix" \
		--replace Microsoft.UI.Composition.=opencomp.dll \
		--replace Microsoft.UI.Dispatching.=opendispatch.dll \
		--out "probe.exe.manifest"
else
	echo "gate-b: no activation manifest (set ACTIVATION_MSIX); activations will fail"
fi

cargo build -q --target x86_64-pc-windows-gnu -p xaml-probe -p opendispatch -p opencomp
# The SDK's own `Microsoft.UI.Dispatching` server aborts on load under Wine
# (`ntdll.NtAllocateReserveObject`), so the manifest points that namespace at ours.
xvfb-run -a "$wine" wineboot -u >/dev/null 2>&1 || true
# The probe and the DLLs sit together and the probe runs from that directory: a DLL
# loaded by full path does not put its own directory on the dependency search path.
cp "$target/xaml-probe.exe" "$run/"
cp "probe.exe.manifest" "$run/xaml-probe.exe.manifest" 2>/dev/null || true

official=0
probe official || official=$?

# Our stand-in for the flat C layer XAML imports. The exports are the 149 names the
# shipped DLL carries; the ones XAML actually imports are all present.
cargo build -q --target x86_64-pc-windows-gnu -p openudk
cp "$target/openudk.dll" "$run/Microsoft.Internal.FrameworkUdk.dll"
# The stand-in run is the one that has to start the engine and measure: with our
# dispatcher in the manifest, `Application.Start` can actually reach its callback.
OPENCOMP_XAML_PROBE_START=1 OPENCOMP_XAML_PROBE_ONLY=Application
export OPENCOMP_XAML_PROBE_START OPENCOMP_XAML_PROBE_ONLY
# Our DLLs go in only for the stand-in run: an old SDK file that finds `bcp47mrm.dll`
# beside it takes a different path than one that does not, and the official run is the
# baseline. `Microsoft.Windows.ApplicationModel.Resources.dll` imports one function from
# `bcp47mrm.dll`, which Wine does not have.
cp "$target/opendispatch.dll" "$run/" 2>/dev/null || true
cp "$target/opencomp.dll" "$run/" 2>/dev/null || true
cp "$target/bcp47mrm.dll" "$run/" 2>/dev/null || true
standin=0
probe standin || standin=$?

echo "gate-b: official exited $official, stand-in exited $standin"
# The stand-in run is the one Gate B needs; the official run is the baseline it is
# compared against.
exit "$standin"
