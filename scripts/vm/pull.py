#!/usr/bin/env python3
"""Fetch one file from the guest.

`push.py` in reverse: a one-shot server on the host accepts a PUT, and the guest
uploads with `curl -T`, which Windows also ships. A directory is zipped first;
`README.md` has the line for that.
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


class Sink(http.server.BaseHTTPRequestHandler):
    directory = "."

    def do_PUT(self):
        name = os.path.basename(self.path.split("?")[0])
        length = int(self.headers.get("Content-Length", 0))
        with open(os.path.join(self.directory, name), "wb") as out:
            out.write(self.rfile.read(length))
        self.send_response(201)
        self.end_headers()

    def log_message(self, *args):
        pass


def pull(remote, directory, port=None):
    port = port or free_port()
    handler = type("Sink", (Sink,), {"directory": directory})
    server = http.server.ThreadingHTTPServer(("", port), handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    name = os.path.basename(remote.replace("\\", "/"))
    try:
        ssh.run(f"cmd /c curl.exe -sS -T {remote} http://10.0.2.2:{port}/{name}")
    finally:
        server.shutdown()
    return os.path.join(directory, name)


if __name__ == "__main__":
    print(pull(sys.argv[1], sys.argv[2] if len(sys.argv) > 2 else "."))
