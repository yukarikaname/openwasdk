import http.server, socketserver, os, json, datetime
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.environ.get("OPENWASDK_SERVE_ROOT") or os.path.join(HERE, "..", "..", "target", "vm", "serve")
RESULTS = os.environ.get("OPENWASDK_RESULTS") or os.path.join(HERE, "..", "..", "target", "vm", "results.log")
class H(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k): super().__init__(*a, directory=ROOT, **k)
    def do_POST(self):
        n = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(n)
        with open(RESULTS, "ab") as f:
            f.write(b"\n=== " + datetime.datetime.now().isoformat().encode() + b" " + self.path.encode() + b"\n")
            f.write(body + b"\n")
        self.send_response(200); self.end_headers(); self.wfile.write(b"ok")
    def log_message(self, fmt, *args): pass
os.makedirs(ROOT, exist_ok=True)
open(os.path.join(ROOT, "ping"), "w").write("pong\n")
socketserver.ThreadingTCPServer.allow_reuse_address = True
with socketserver.ThreadingTCPServer(("0.0.0.0", 8000), H) as s: s.serve_forever()
