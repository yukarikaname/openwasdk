#!/usr/bin/env python3
"""Run the gate-b probe in the guest and print what it logged.

The probe needs four environment variables and a working directory, which is
more quoting than a one-line `ssh` command survives; the log is written to a
file first so a crash cannot take the output with it.
"""
import sys

import ssh

PROBE = (
    r"cd /d c:\base"
    r" & set XAML_DLL=c:\base\Microsoft.ui.xaml.dll"
    r" & set OPENCOMP_TRACE=1"
    r" & set OPENCOMP_XAML_PROBE_START=1"
    r" & set OPENCOMP_XAML_PROBE_WINDOW=1"
    r" & xaml-probe.exe > {log} 2>&1"
    r" & type {log}"
)


def run(log=r"c:\probe.log", timeout=120):
    return ssh.run(PROBE.format(log=log), timeout=timeout)


if __name__ == "__main__":
    sys.stdout.write(run(*(sys.argv[1:] or [])))
