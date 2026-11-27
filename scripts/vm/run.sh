#!/usr/bin/env bash
# Boot the Windows 11 ARM64 baseline VM under QEMU/HVF.
#
#   scripts/vm/run.sh [vm-dir]
#
# The baseline is the other half of every Wine question: the engine's private interfaces
# only exist on Windows, so what they return has to be measured there. `autounattend.iso`
# carries the answer file that bypasses the TPM check, skips OOBE, logs in as `dev`, and
# pulls the probe from the host at `10.0.2.2:8000` on first logon.
#
# Needs, in the VM directory: `Win11_arm64.iso`, `autounattend.iso`, `efi.fd` (a writable
# copy of `share/qemu/edk2-aarch64-code.fd`), and `win11.qcow2`.
set -euo pipefail
vm=${1:-${VM_DIR:-$HOME/openwasdk-vm}}
for file in Win11_arm64.iso autounattend.iso efi.fd win11.qcow2; do
	[ -f "$vm/$file" ] || { echo "vm: missing $vm/$file" >&2; exit 2; }
done
exec qemu-system-aarch64 \
	-machine virt,highmem=on -accel hvf -cpu host -smp 4 -m 4096 \
	-drive if=pflash,format=raw,file="$vm/efi.fd" \
	-device qemu-xhci -device usb-kbd -device usb-tablet \
	-device nvme,drive=hd0,serial=hd0 \
	-drive if=none,id=hd0,file="$vm/win11.qcow2",format=qcow2 \
	-drive if=none,id=cd0,file="$vm/Win11_arm64.iso",media=cdrom,readonly=on \
	-device usb-storage,drive=cd0 \
	-drive if=none,id=cd1,file="$vm/autounattend.iso",media=cdrom,readonly=on \
	-device usb-storage,drive=cd1 \
	-device virtio-gpu-pci -display cocoa,show-cursor=on \
	-device virtio-net-pci,netdev=n0 -netdev user,id=n0 \
	-device virtio-rng-pci \
	-boot order=d
