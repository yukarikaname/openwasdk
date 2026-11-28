#!/bin/sh
# Copy wine-uwp's ALPC subset onto a clean Wine tree.
#
# wine-uwp carries 547 changed files against Wine 11.22, almost all of them for the
# UWP app model. What the CoreMessaging registrar actually needs is this list: the
# server-side port object, the client side that talks to it, the request numbers
# that connect them, and the wait-completion-packet implementation in `sync.c`.
#
# usage: apply-alpc.sh <wine-uwp-checkout> <wine-11.22-source>
set -e

uwp=${1:?usage: apply-alpc.sh <wine-uwp-checkout> <wine-11.22-source>}
wine=${2:?usage: apply-alpc.sh <wine-uwp-checkout> <wine-11.22-source>}

files='
include/winternl.h
server/alpc.c
server/protocol.def
server/queue.c
server/request_handlers.h
server/request_trace.h
server/token.c
dlls/ntdll/ntsyscalls.h
dlls/ntdll/ntdll.spec
dlls/ntdll/sync.c
dlls/ntdll/unix/alpc.c
dlls/wow64/syscall.c
'

for file in $files; do
	if [ ! -f "$uwp/wine/$file" ]; then
		echo "missing in the wine-uwp tree: $file" >&2
		exit 1
	fi
	cp "$uwp/wine/$file" "$wine/$file"
	echo "applied $file"
done
