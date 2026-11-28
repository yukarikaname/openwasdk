#!/bin/sh
# Build DXMT against this project's Wine, so macOS gets D3D11 on Metal.
#
# wined3d's OpenGL backend cannot offer the D3D feature levels XAML asks for; its Vulkan
# backend needs the MoltenVK plumbing to line up exactly. DXMT replaces d3d11/dxgi with a
# Metal implementation instead, which reports D3D_FEATURE_LEVEL_11_1 out of the box.
#
# DXMT's prebuilt releases do not load in a Wine of a different generation: the PE side
# needs its unixlib companion installed as a builtin, and a prefix that keeps its own
# copies of d3d11.dll cannot be talked into using it. The reliable route is an installed
# Wine plus `meson install`.
#
# usage: build-dxmt.sh <wine-source> <wine-build> <work-dir>
set -e

wine_src=${1:?usage: build-dxmt.sh <wine-source> <wine-build> <work-dir>}
wine_build=${2:?usage: build-dxmt.sh <wine-source> <wine-build> <work-dir>}
work=${3:?usage: build-dxmt.sh <wine-source> <wine-build> <work-dir>}

# Homebrew keeps these two out of /usr/bin: bison there is 2.3, and mingw-w64 is not
# installed at all. Take them from brew when it exists, so a machine that provides them
# another way only has to put them on PATH.
if command -v brew >/dev/null 2>&1; then
	PATH="$(brew --prefix bison)/bin:$(brew --prefix mingw-w64)/bin:$PATH"
	export PATH
fi

# 1. LLVM 15, x86_64, the exact major DXMT wants.
if [ ! -x "$work/toolchains/llvm/bin/llvm-config" ]; then
	mkdir -p "$work/toolchains"
	[ -d "$work/toolchains/llvm-project" ] ||
		git clone --depth 1 --branch llvmorg-15.0.7 https://github.com/llvm/llvm-project.git "$work/toolchains/llvm-project"
	cmake -B "$work/toolchains/llvm-build" -S "$work/toolchains/llvm-project/llvm" \
		-DCMAKE_INSTALL_PREFIX="$work/toolchains/llvm" \
		-DCMAKE_OSX_ARCHITECTURES=x86_64 -DLLVM_HOST_TRIPLE=x86_64-apple-darwin \
		-DLLVM_ENABLE_ASSERTIONS=On -DLLVM_ENABLE_ZSTD=Off -DCMAKE_BUILD_TYPE=Release \
		-DLLVM_TARGETS_TO_BUILD="" -DLLVM_BUILD_TOOLS=Off -G Ninja
	cmake --build "$work/toolchains/llvm-build"
	cmake --install "$work/toolchains/llvm-build"
fi

# 2. An installed Wine: DXMT installs into the `lib/wine` layout, and a prefix's own
#    copies of d3d11.dll win over anything else.
install=$work/wine-install/usr/local
if [ ! -x "$install/bin/wine" ]; then
	rm -rf "$work/wine-install"
	make -C "$wine_build" install DESTDIR="$work/wine-install"
	# `make install` does not ship winebuild, and DXMT's meson asks for it.
	cp "$wine_build/tools/winebuild/winebuild" "$install/bin/"
fi

# 3. DXMT itself.
if [ ! -d "$work/DXMT" ]; then
	git clone --depth 1 https://github.com/3Shain/dxmt.git "$work/DXMT"
	(cd "$work/DXMT" && git submodule update --init --recursive)
fi
cd "$work/DXMT"
meson setup --cross-file build-win64.txt --prefix="$install" \
	-Dnative_llvm_path="$work/toolchains/llvm" -Dwine_install_path="$install" \
	build-install --buildtype release
meson compile -C build-install
meson install -C build-install

# The unixlib and the PE side have to land in the same layout Wine loads builtins from.
cp src/winemetal/unix/winemetal.so "$install/lib/wine/x86_64-unix/" 2>/dev/null || true
cp src/winemetal/winemetal.dll src/dxgi/dxgi.dll src/d3d11/d3d11.dll src/d3d10/d3d10core.dll \
	"$install/lib/wine/x86_64-windows/" 2>/dev/null || true
rm -rf "$install/x86_64-windows"
echo "DXMT installed into $install"
