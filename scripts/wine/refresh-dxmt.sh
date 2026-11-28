#!/bin/sh
# Put DXMT back after `make install` has replaced it with wined3d.
#
# `make install` copies Wine's own d3d11, dxgi and d3d10core into the install tree, so a
# rebuild silently returns the engine to a feature level XAML does not accept. Re-running
# meson install is the correct fix but takes minutes; copying the built files is enough,
# because the DXMT build is already up to date.
#
# usage: refresh-dxmt.sh <dxmt-build-dir> <wine-install>
set -e

dxmt=${1:?usage: refresh-dxmt.sh <dxmt-build-dir> <wine-install>}
install=${2:?usage: refresh-dxmt.sh <dxmt-build-dir> <wine-install>}
windows=$install/lib/wine/x86_64-windows
unix=$install/lib/wine/x86_64-unix

for dll in d3d11 dxgi d3d10core winemetal; do
	[ -f "$dxmt/src/$dll/$dll.dll" ] || [ -f "$dxmt/src/winemetal/winemetal.dll" ] ||
		{ echo "no $dll.dll under $dxmt/src" >&2; exit 1; }
done
[ -f "$dxmt/src/winemetal/unix/winemetal.so" ] || { echo "no winemetal.so" >&2; exit 1; }

cp "$dxmt/src/d3d11/d3d11.dll" "$dxmt/src/dxgi/dxgi.dll" \
	"$dxmt/src/d3d10/d3d10core.dll" "$dxmt/src/winemetal/winemetal.dll" "$windows"/
cp "$dxmt/src/winemetal/unix/winemetal.so" "$unix"/

echo "DXMT restored in $install"
ls -l "$windows/d3d11.dll" "$unix/winemetal.so" | awk '{print "  " $5, $9}'
