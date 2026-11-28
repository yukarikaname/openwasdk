#!/bin/sh
# Configure a patched Wine 11.22 to run x86_64 Windows binaries on macOS arm64.
#
# The unix side is arm64 and the PE side is x86_64, which is Wine's "new wow64" and
# what the prebuilt Wine here does. Only x86_64 is enabled: mingw-w64 provides both
# targets, but nothing in this project needs 32-bit Windows binaries.
#
# usage: build-macos.sh <wine-source> [make -j level]
set -e

src=${1:?usage: build-macos.sh <wine-source> [jobs]}
jobs=${2:-8}
shift 2 2>/dev/null || shift $#
mingw=/opt/homebrew/Cellar/mingw-w64/14.0.0_3/bin

# macOS ships bison 2.3; the expression parser Wine generates needs 3.0 or newer.
PATH=/opt/homebrew/opt/bison/bin:$PATH
export PATH

cd "$src"
./configure \
	--enable-archs=x86_64 \
	--with-mingw="$mingw/x86_64-w64-mingw32-gcc" \
	--without-x \
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
	--disable-tests \
	"$@"

make -j"$jobs"
