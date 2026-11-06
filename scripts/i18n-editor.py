#!/usr/bin/env python3
"""Edit Chyk's interface text in the browser, with a live preview of the Phosphor mockups.

Usage: python3 scripts/i18n-editor.py [port]

Serves on 127.0.0.1 only and writes nothing but assets/i18n/<lang>.lang. Rebuild Chyk afterwards:
the strings are compiled into the app.
"""
import json
import sys
import webbrowser
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LANG_DIR = ROOT / "assets/i18n"
LANGS = ("en", "uk")
EDITOR = Path(__file__).resolve().parent / "i18n-editor"
# The editor page and its parts; every other path is served from the mockups.
PAGES = {
    "/": "index.html",
    "/editor": "index.html",
    "/editor/style.css": "style.css",
    "/editor/table.js": "table.js",
    "/editor/preview.js": "preview.js",
}
TYPES = {".html": "text/html", ".css": "text/css", ".js": "text/javascript"}
MOCKUPS = ROOT / "design/concepts"


def save(lang, values):
    """Rewrite `lang`.lang in place: keep comments and order, update values, append new keys."""
    path = LANG_DIR / f"{lang}.lang"
    clean = {str(k).strip(): " ".join(str(v).split()) for k, v in values.items() if str(k).strip()}
    out, seen = [], set()
    for line in path.read_text(encoding="utf-8").splitlines():
        s = line.strip()
        if s and not s.startswith("#") and "=" in s:
            key = s.split("=", 1)[0].strip()
            if key in clean:
                out.append(f"{key} = {clean[key]}".rstrip())
                seen.add(key)
                continue
        out.append(line)
    new = [k for k in clean if k not in seen and clean[k]]
    if new:
        out += ["", "# Added in the editor"] + [f"{k} = {clean[k]}" for k in new]
    tmp = path.with_suffix(".tmp")
    tmp.write_text("\n".join(out) + "\n", encoding="utf-8")
    tmp.replace(path)


class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(MOCKUPS), **kwargs)

    def log_message(self, *args):
        pass

    def send_json(self, code, body):
        data = json.dumps(body, ensure_ascii=False).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if self.path in PAGES:
            path = EDITOR / PAGES[self.path]
            data = path.read_bytes()
            self.send_response(200)
            self.send_header("Content-Type", f"{TYPES[path.suffix]}; charset=utf-8")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
        elif self.path == "/api/strings":
            self.send_json(200, {l: (LANG_DIR / f"{l}.lang").read_text(encoding="utf-8") for l in LANGS})
        else:
            super().do_GET()  # mockup HTML and assets from design/concepts

    def do_POST(self):
        if self.path != "/api/save":
            return self.send_json(404, {"error": "not found"})
        # Only same-origin pages may write; blocks other sites posting to localhost.
        host = self.headers.get("Host", "")
        origin = self.headers.get("Origin", f"http://{host}")
        if origin != f"http://{host}":
            return self.send_json(403, {"error": "forbidden"})
        try:
            body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))))
            for lang, values in body.items():
                if lang not in LANGS or not isinstance(values, dict):
                    raise ValueError(f"bad language {lang!r}")
                save(lang, values)
        except (ValueError, json.JSONDecodeError) as e:
            return self.send_json(400, {"error": str(e)})
        self.send_json(200, {"ok": True})


def main():
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8765
    server = ThreadingHTTPServer(("127.0.0.1", port), Handler)
    url = f"http://127.0.0.1:{port}/"
    print(f"Chyk translation editor: {url}  (Ctrl+C to stop)")
    webbrowser.open(url)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
