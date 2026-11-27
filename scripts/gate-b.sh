#!/usr/bin/env bash
# Load the official Microsoft.ui.xaml.dll under Wine and report what it answers.
#
# Gate B starts here. The DLL is the real one, next to the DLLs it ships with; nothing
# of ours is in front of it yet, so the first runs are a measurement of what the engine
# asks for. The same script runs on the CI runner and in a local container, which is
# why it takes the SDK directory as an argument instead of assuming a layout.
#
#   scripts/gate-b.sh [sdk-dir]
#
# `OPENCOMP_SDK_FETCH=1` downloads the packages when the directory is empty. Set
# `NUGET_BASE` to a mirror to make that download bearable outside a datacenter; the
# flat-container path below is the same on nuget.org and on the mirrors that proxy it.

set -euo pipefail

sdk=${1:-target/appsdk}
version=${SDK_VERSION:-2.7.0}
base=${NUGET_BASE:-https://api.nuget.org/v3-flatcontainer}
wine=$(command -v wine64 || echo /usr/lib/wine/wine64)

# The native files are under `runtimes-framework/win-x64/native`, and one load needs
# four packages: WinUI has the engine, Foundation the runtime it starts, and
# InteractiveExperiences carries Microsoft.UI.dll, dcompi.dll and the FrameworkUdk
# stand-in. DWrite is the text engine XAML links against.
fetch() {
	mkdir -p "$sdk"
	for pkg in winui foundation interactiveexperiences dwrite; do
		name="microsoft.windowsappsdk.$pkg"
		curl -sSLf -o "$pkg.nupkg" \
			"$base/$name/$version/$name.$version.nupkg"
		unzip -Z1 "$pkg.nupkg" |
			grep -E '^runtimes-framework/win-x64/native/[^/]+\.dll$' > files.txt
		unzip -j -o -q "$pkg.nupkg" -d "$sdk" $(tr '\n' ' ' < files.txt)
		rm -f "$pkg.nupkg" files.txt
	done
}

if [ ! -f "$sdk/Microsoft.ui.xaml.dll" ] || [ "${OPENCOMP_SDK_FETCH:-0}" = 1 ]; then
	fetch
fi
count=$(find "$sdk" -maxdepth 1 -name '*.dll' | wc -l)
echo "gate-b: $count SDK DLLs in $sdk"

export WINEPREFIX=${WINEPREFIX:-$PWD/target/wine}
export WINEDLLOVERRIDES=${WINEDLLOVERRIDES:-mscoree,mshtml=}
export RUST_BACKTRACE=1
cargo build -q --target x86_64-pc-windows-gnu -p xaml-probe
xvfb-run -a "$wine" wineboot -u >/dev/null 2>&1 || true

# Run from the SDK directory: a DLL loaded by full path does not put its own directory
# on the dependency search path, and the current directory is searched. `err+loaddll`
# names the module that is missing when the load fails, which is the only failure mode
# that does not print an HRESULT.
cp target/x86_64-pc-windows-gnu/debug/xaml-probe.exe "$sdk/"
export XAML_DLL="$PWD/$sdk/Microsoft.ui.xaml.dll"
export OPENCOMP_XAML_PROBE_JSON=1
export WINEDEBUG=${WINEDEBUG:-err+loaddll}

cd "$sdk"
set +e
timeout -k 5 120 xvfb-run -a timeout -k 5 120 "$wine" ./xaml-probe.exe 2>&1 | tee probe.log
status=${PIPESTATUS[0]}
set -e
echo "gate-b: probe exited $status"
exit "$status"
