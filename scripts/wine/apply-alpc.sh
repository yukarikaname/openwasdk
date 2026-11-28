#!/bin/sh
# Apply wine-uwp's ALPC subset onto a clean Wine 11.22 tree.
#
# wine-uwp carries 547 changed files against Wine 11.22, almost all of them for the
# UWP app model. What the CoreMessaging registrar needs is much less, and finding the
# boundary took three link failures; this script is the boundary.
#
# Left out on purpose:
#   dlls/ntdll/ntsyscalls.h, dlls/wow64/syscall.c
#       Their versions renumber every syscall, because their client also adds
#       NtAlpcConnectPortEx and NtAlpcQueryInformation. We do not export those, so the
#       upstream numbering stands and these two files must not be touched.
#   dlls/ntdll/ntdll.spec
#       Only the three wait-completion-packet entry points are wanted, and they are not
#       syscalls -- their implementation lives in sync.c on the PE side.
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
dlls/ntdll/sync.c
dlls/ntdll/unix/alpc.c
'

for file in $files; do
	if [ ! -f "$uwp/wine/$file" ]; then
		echo "missing in the wine-uwp tree: $file" >&2
		exit 1
	fi
	cp "$uwp/wine/$file" "$wine/$file"
	echo "applied $file"
done

python3 - "$wine" <<'PY'
import sys
from pathlib import Path

wine = Path(sys.argv[1])

spec = wine / "dlls/ntdll/ntdll.spec"
text = spec.read_text()
if "NtCreateWaitCompletionPacket" not in text:
    anchor = "@ stdcall -syscall NtCreateTimer("
    add = (
        "@ stdcall NtAssociateWaitCompletionPacket(long long long ptr ptr long long ptr)\n"
        "@ stdcall NtCancelWaitCompletionPacket(long long)\n"
        "@ stdcall NtCreateWaitCompletionPacket(ptr long ptr)\n"
    )
    out = []
    for line in text.splitlines(keepends=True):
        out.append(line)
        if line.startswith(anchor):
            out.append(add)
    spec.write_text("".join(out))
    print("applied three wait-completion-packet exports")

loader = wine / "dlls/ntdll/loader.c"
text = loader.read_text()
if "ldr_dllmain_in_progress" not in text:
    loader.write_text(text + '''
/***********************************************************************
 *           ldr_dllmain_in_progress   (NTDLL.@)
 *
 * Which module's DllMain the given thread is inside, or NULL when it is not
 * inside one. The answer is only used to explain a critical-section timeout
 * in one log line, so a tree that does not track the transition -- this one
 * does not -- can honestly say "nothing".
 */
const WCHAR *ldr_dllmain_in_progress( DWORD tid, ULONG *reason )
{
    if (reason) *reason = 0;
    return NULL;
}
''')
    print("applied the DllMain diagnostic helper")
PY
