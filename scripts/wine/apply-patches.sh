#!/bin/sh
# Put this project's changes into a clean Wine source tree.
#
# The series is what makes the Wine side reproducible. Every change is a numbered patch
# against wine-11.22, so a tree can be rebuilt from a tarball or a fresh clone instead of
# from whichever files happened to be edited by hand; the alternative used to be two
# apply scripts for the parts copied from wine-uwp plus a memory of the rest.
#
# usage: apply-patches.sh <wine-source>
set -e

src=${1:?usage: apply-patches.sh <wine-source>}
dir=$(cd "$(dirname "$0")" && pwd)

for patch_file in "$dir"/patches/*.patch; do
	echo "applying $(basename "$patch_file")"
	patch -p1 -s --batch -d "$src" < "$patch_file" || {
		echo "failed: $patch_file" >&2
		exit 1
	}
done
echo "applied $(ls "$dir"/patches/*.patch | wc -l | tr -d ' ') patches to $src"
