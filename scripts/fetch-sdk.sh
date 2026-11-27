#!/usr/bin/env bash
# Put the official Windows App SDK binaries in one directory.
#
# Four packages make one loadable set: `WinUI` has `Microsoft.ui.xaml.dll`,
# `Foundation` has the runtime it starts, `InteractiveExperiences` has
# `Microsoft.UI.dll`, `dcompi.dll` and the `Microsoft.Internal.FrameworkUdk.dll`
# stand-in target, and `DWrite` has the text engine XAML links against. The native
# files sit under `runtimes-framework/win-x64/native` for the framework DLLs and under
# `runtimes/win-x64/native` for the bootstrapper.
#
#   scripts/fetch-sdk.sh [sdk-dir]
#
# `NUGET_BASE` points the download at another flat-container host; the path below it is
# the same as on nuget.org.

set -euo pipefail

sdk=${1:-target/appsdk}
version=${SDK_VERSION:-2.7.0}
base=${NUGET_BASE:-https://api.nuget.org/v3-flatcontainer}

mkdir -p "$sdk"
for pkg in winui foundation interactiveexperiences dwrite; do
	name="microsoft.windowsappsdk.$pkg"
	curl -sSLf -o "$pkg.nupkg" "$base/$name/$version/$name.$version.nupkg"
	unzip -Z1 "$pkg.nupkg" |
		grep -E '^runtimes(-framework)?/win-x64/native/[^/]+\.dll$' > files.txt
	unzip -j -o -q "$pkg.nupkg" -d "$sdk" $(tr '\n' ' ' < files.txt)
	rm -f "$pkg.nupkg" files.txt
done
echo "fetch-sdk: $(find "$sdk" -maxdepth 1 -name '*.dll' | wc -l) DLLs in $sdk"
