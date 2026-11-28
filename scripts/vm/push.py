#!/usr/bin/env python3
"""Copy one file into the guest.

The guest reaches the host as 10.0.2.2 and `curl.exe` is part of Windows, so a
one-shot HTTP server is the whole transport. `scp` is not used: the account is
password-only and there is no sshpass on this side.
"""
import http.server
import os
import socket
import sys
import threading

import ssh


def free_port():
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass


def serve(directory, port):
    handler = lambda *a, **k: Quiet(*a, directory=directory, **k)
    server = http.server.ThreadingHTTPServer(("", port), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def push(local, remote, port=None):
    directory = os.path.dirname(os.path.abspath(local))
    name = os.path.basename(local)
    port = port or free_port()
    server = serve(directory, port)
    try:
        ssh.run(f'cmd /c curl.exe -sS -o %TEMP%\\{name} http://10.0.2.2:{port}/{name} & '
                f'attrib -r {remote} & move /y %TEMP%\\{name} {remote}')
    finally:
        server.shutdown()


if __name__ == "__main__":
    push(sys.argv[1], sys.argv[2])
