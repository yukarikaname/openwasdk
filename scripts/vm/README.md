# The Windows baseline VM

Every private interface this project has to answer was discovered by asking Windows. The
VM is that "asking" half: the same probe binary, the same SDK, on the platform where the
engine's own implementation exists.

- `run.sh` boots it under QEMU with HVF. The hardware is `virt` with NVMe, virtio-gpu,
  user-mode networking, and a writable copy of QEMU's aarch64 EDK2 image.
- `autounattend.xml` is the answer file on the second CD. It bypasses the TPM and Secure
  Boot checks, skips OOBE, creates `dev`/`dev`, and runs one command at first logon.
- `boot.ps1` is what that command fetches: it downloads `gate-b.zip` from the host at
  `10.0.2.2:8000`, runs the probe in both modes, and posts the output back.
- `serve.py` is the host side: it serves the VM directory and writes every POST to
  `results.log`.

The ISO is Microsoft's own ARM64 multi-edition image, downloaded from the Windows 11
download page; it is not committed.
