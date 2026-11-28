#!/usr/bin/env python3
"""Run one command in the baseline VM over SSH.

The guest's only account is `dev` with the password the answer file sets, and there is no
sshpass here: a pty answers the prompt instead. The first matching prompt is
`'s password:` -- the server's capability line also contains the word "password", and
answering that one loses the race.
"""
import os, pty, select, sys, time

def run(command, port="2222", user="dev", password="dev", timeout=120):
    pid, fd = pty.fork()
    if pid == 0:
        os.execvp("ssh", ["ssh", "-o", "StrictHostKeyChecking=no",
                          "-o", "UserKnownHostsFile=/dev/null",
                          "-o", "PreferredAuthentications=password",
                          "-o", "PubkeyAuthentication=no",
                          "-o", "ConnectTimeout=10",
                          "-p", port, f"{user}@127.0.0.1", command])
    out, sent, end = b"", False, time.time() + timeout
    while time.time() < end:
        ready, _, _ = select.select([fd], [], [], 0.5)
        if ready:
            try:
                data = os.read(fd, 4096)
            except OSError:
                break
            if not data:
                break
            out += data
            if not sent and b"'s password:" in out:
                time.sleep(0.3)
                os.write(fd, password.encode() + b"\n")
                sent = True
    return out.decode(errors="replace")

if __name__ == "__main__":
    # Long builds outlive the default; the caller says how long it is willing to wait.
    budget = int(os.environ.get("SSH_TIMEOUT", "120"))
    sys.stdout.write(run(" ".join(sys.argv[1:]), timeout=budget))
