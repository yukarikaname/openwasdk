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

## Reaching the guest

`ssh.py` runs one command over `hostfwd` (host port 2222) as `dev`/`dev`. Password auth is
answered from a pty: there is no sshpass on the host, and the server's capability line
contains the word "password" too, so the first prompt that counts is `'s password:`.

The guest needs a network card first. Windows 11 ARM64 has no driver for `e1000`, `e1000e`,
`rtl8139` or QEMU's `usb-net` (which is CDC-ECM, not RNDIS), so the VM boots with
`virtio-net-pci` and `pnputil /add-driver` installs `NetKVM/w11/ARM64` from the virtio-win
ISO. `OpenSSH.Server` then comes from Windows Update over the slirp network.

## Working in the guest

`push.py <local> <remote>` copies one file in: the guest reaches the host as `10.0.2.2`
and `curl.exe` is part of Windows, so a one-shot HTTP server is the whole transport.
`probe.py` runs the gate-b window probe with the environment it needs and prints its log.
Between them, one iteration is two commands.

UAC is off in the guest (`EnableLUA=0`, written to the registry and then rebooted), so an
SSH session arrives with a high-integrity token: `pnputil`, `winget` and the SDK installer
work without a prompt on the desktop.

Debugging uses the command-line debugger from the Windows SDK, not the WinDbg MSIX: the
MSIX download host answers 403 from here, while `winsdksetup.exe /features
OptionId.WindowsDesktopDebuggers /q` works. The debugger lands in
`Debuggers\arm64\cdb.exe` (the guest is ARM64, so the x64 folder stays empty).

`cdb` is driven from a command file; quoting a command list through `cmd` loses the
semicolons. A session that stops on the fatal exception only, and then shows the stack:

    sxd 0xc0000008
    sxd 0x80000003
    sxe 0xc000027b
    sxe 0xc0000409
    g
    .exr -1
    !analyze -v
    kb 40
    q

`0xc0000008` and `0x80000003` are first-chance noise on this image; leaving them armed
stops the debugger long before the interesting event.
