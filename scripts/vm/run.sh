#!/usr/bin/env bash
# Boot the Windows 11 ARM64 baseline VM under QEMU/HVF.
#
#   scripts/vm/run.sh [vm-dir]      (default: $VM_DIR or <repo>/target/vm)
#
# The baseline is the other half of every Wine question: the engine's private interfaces
# only exist on Windows, so what they return has to be measured there. `autounattend.iso`
# carries the answer file that bypasses the TPM check, skips OOBE, logs in as `dev`, and
# pulls the probe from the host at `10.0.2.2:8000` on first logon.
#
# Needs, in the VM directory: `Win11_arm64.iso`, `autounattend.iso`, `efi.fd` (a writable
# copy of `share/qemu/edk2-aarch64-code.fd`), and `win11.qcow2`.
set -euo pipefail
here=$(cd "$(dirname "$0")/../.." && pwd)
vm=${1:-${VM_DIR:-$here/target/vm}}
for file in Win11_arm64.iso autounattend.iso efi.fd win11.qcow2; do
	[ -f "$vm/$file" ] || { echo "vm: missing $vm/$file" >&2; exit 2; }
done
qemu-system-aarch64 \
	-machine virt -accel hvf -cpu host -smp 4 -m 4096 \
	-drive if=pflash,format=raw,file="$vm/efi.fd" \
	-device qemu-xhci \
	-device usb-kbd -device usb-tablet \
	-device nvme,drive=hd0,serial=hd0 \
	-drive if=none,id=hd0,file="$vm/win11.qcow2",format=qcow2 \
	-device usb-storage,drive=cd0 \
	-drive if=none,id=cd0,format=raw,media=cdrom,readonly=on,file="$vm/Win11_arm64.iso" \
	-device usb-storage,drive=cd1 \
	-drive if=none,id=cd1,format=raw,media=cdrom,readonly=on,file="$vm/autounattend.iso" \
	-device ramfb -display cocoa,show-cursor=on \
	-nic user,model="${VM_NIC:-e1000}" \
	-device virtio-rng-pci \
	-monitor unix:"$vm/mon.sock",server,nowait \
	-boot order="${VM_BOOT:-d}" &
pid=$!
# The install media carries `bootfix.bin`: it asks for a key and falls through to PXE if
# none arrives, so the first minute is spent answering that prompt.
for _ in $(seq 1 45); do
	[ -S "$vm/mon.sock" ] && printf 'sendkey spc\n' | nc -U "$vm/mon.sock" >/dev/null 2>&1
	sleep 2
done
wait "$pid"
