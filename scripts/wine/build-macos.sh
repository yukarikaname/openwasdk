#!/bin/sh
# Build a patched Wine 11.22 that runs x86_64 Windows binaries on macOS.
#
# The unix side has to be x86_64 as well, not the arm64 host: macOS runs x86_64 through
# Rosetta, and every Wine that works on this machine is x86_64 on both sides. Building the
# unix side for the host produces the Windows-on-ARM shape instead, which links, starts,
# and then dies with no output at all.
#
# So this is a cross build: unix objects with `clang -arch x86_64`, PE objects with mingw,
# and the build tools taken from an arm64 tree because `winebuild`, `widl` and `wmc` have to
# run while the tree is being built for something else.
#
# Freetype is off: the only x86_64 one available would have to be built by hand, and the
# text in these applications is drawn by the App SDK's DWrite, not by Wine. An out-of-tree
# build also regenerates `server_protocol.h` from `protocol.def`, which is what an in-tree
# build of a release tarball does not do on its own.
#
# usage: build-macos.sh <wine-source> <build-dir> [jobs]
set -e

src=${1:?usage: build-macos.sh <wine-source> <build-dir> [jobs]}
build=${2:?usage: build-macos.sh <wine-source> <build-dir> [jobs]}
jobs=${3:-8}

# macOS ships bison 2.3; the expression parser Wine generates needs 3.0 or newer.
PATH=/opt/homebrew/opt/bison/bin:/opt/homebrew/Cellar/mingw-w64/14.0.0_3/bin:$PATH
export PATH

export CC="clang -arch x86_64"
export CXX="clang++ -arch x86_64"
export OBJC="clang -arch x86_64"
export OBJCXX="clang++ -arch x86_64"

mkdir -p "$build"
cd "$build"

"$src/configure" \
	--host=x86_64-apple-darwin \
	--build=aarch64-apple-darwin \
	--with-wine-tools="$src" \
	--without-x \
	--without-freetype \
	--without-alsa \
	--without-capi \
	--without-cups \
	--without-dbus \
	--without-gphoto \
	--without-krb5 \
	--without-netapi \
	--without-opencl \
	--without-oss \
	--without-pcap \
	--without-pulse \
	--without-sane \
	--without-udev \
	--without-usb \
	--without-v4l2 \
	--disable-tests

make -j"$jobs"
