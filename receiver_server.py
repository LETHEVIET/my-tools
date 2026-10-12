"""Small authenticated LAN photo receiver. No third-party web-server dependency."""
from __future__ import annotations

from collections import defaultdict, deque
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import hmac
import json
from pathlib import Path
import secrets
import threading
import time
from urllib.parse import urlsplit

from image_processing import MAX_UPLOAD_BYTES, InvalidImage, load_and_normalize

WEB_ROOT = Path(__file__).resolve().parent / "web"
ALLOWED_FILES = {
    "/": "index.html", "/index.html": "index.html", "/app.css": "app.css",
    "/app.js": "app.js", "/sw.js": "sw.js", "/manifest.webmanifest": "manifest.webmanifest",
    "/icons/icon-192.png": "icons/icon-192.png",
    "/icons/icon-512.png": "icons/icon-512.png",
}
CONTENT_TYPES = {
    ".html": "text/html; charset=utf-8", ".css": "text/css; charset=utf-8",
    ".js": "text/javascript; charset=utf-8", ".webmanifest": "application/manifest+json",
    ".png": "image/png",
}


class AppState:
    def __init__(self, on_photo):
        self.token = secrets.token_urlsafe(32)
        self.on_photo = on_photo
        self.auto_paste = True
        self.received = 0
        self._lock = threading.RLock()
        self._upload_lock = threading.Lock()
        self._requests = defaultdict(deque)
        self.last_result = "Waiting for a photo"

    def rotate_token(self):
        with self._lock:
            self.token = secrets.token_urlsafe(32)
            self._requests.clear()
            return self.token

    def valid_token(self, token):
        with self._lock:
            return bool(token) and hmac.compare_digest(token, self.token)

    def allow_upload(self, peer):
        now = time.monotonic()
        with self._lock:
            recent = self._requests[peer]
            while recent and recent[0] < now - 60:
                recent.popleft()
            if len(recent) >= 12:
                return False
            recent.append(now)
            return True

    def receive(self, image):
        # Serializes clipboard + SendInput so concurrent uploads cannot interleave.
        with self._upload_lock:
            result = self.on_photo(image)
            with self._lock:
                self.received += 1
                self.last_result = result
            return result


class SnapHandler(BaseHTTPRequestHandler):
    server_version = "SnapToPrompt/0.1"

    @property
    def state(self) -> AppState:
        return self.server.app_state

    def log_message(self, fmt, *args):
        # No URLs/tokens in logs.
        pass

    def _headers(self, code, mime="application/json; charset=utf-8", length=0):
        self.send_response(code)
        self.send_header("Content-Type", mime)
        self.send_header("Content-Length", str(length))
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Referrer-Policy", "no-referrer")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' blob: data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'")
        self.end_headers()

    def _json(self, code, payload):
        data = json.dumps(payload).encode("utf-8")
        self._headers(code, length=len(data))
        self.wfile.write(data)

    def _authorized(self):
        header = self.headers.get("Authorization", "")
        if not header.startswith("Bearer "):
            return False
        return self.state.valid_token(header[7:])

    def do_GET(self):
        path = urlsplit(self.path).path
        if path == "/api/status":
            if not self._authorized():
                self._json(HTTPStatus.UNAUTHORIZED, {"error": "Not paired. Scan the new QR code."})
                return
            with self.state._lock:
                self._json(HTTPStatus.OK, {"paired": True, "auto_paste": self.state.auto_paste,
                                           "received": self.state.received, "last_result": self.state.last_result})
            return
        filename = ALLOWED_FILES.get(path)
        if not filename:
            self._json(HTTPStatus.NOT_FOUND, {"error": "Not found"})
            return
        data = (WEB_ROOT / filename).read_bytes()
        self._headers(HTTPStatus.OK, CONTENT_TYPES[Path(filename).suffix], len(data))
        self.wfile.write(data)

    def do_POST(self):
        if urlsplit(self.path).path != "/api/photo":
            self._json(HTTPStatus.NOT_FOUND, {"error": "Not found"})
            return
        if not self._authorized():
            self._json(HTTPStatus.UNAUTHORIZED, {"error": "Pairing expired. Scan your PC QR code again."})
            return
        if not self.state.allow_upload(self.client_address[0]):
            self._json(HTTPStatus.TOO_MANY_REQUESTS, {"error": "Too many photos. Try again shortly."})
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
        except ValueError:
            length = 0
        if length <= 0:
            self._json(HTTPStatus.LENGTH_REQUIRED, {"error": "No photo attached."})
            return
        if length > MAX_UPLOAD_BYTES:
            self._json(HTTPStatus.REQUEST_ENTITY_TOO_LARGE, {"error": "Maximum photo size is 16 MB."})
            return
        raw = self.rfile.read(length)
        if len(raw) != length:
            self._json(HTTPStatus.BAD_REQUEST, {"error": "Incomplete upload"})
            return
        try:
            image = load_and_normalize(raw)
            outcome = self.state.receive(image)
        except InvalidImage as exc:
            self._json(HTTPStatus.UNSUPPORTED_MEDIA_TYPE, {"error": str(exc)})
            return
        except Exception as exc:
            # Don't leak server exceptions into the mobile app.
            self._json(HTTPStatus.CONFLICT, {"error": str(exc)[:180] or "Clipboard was unavailable"})
            return
        self._json(HTTPStatus.OK, {"ok": True, "message": outcome, "width": image.width, "height": image.height})


def make_server(state: AppState, bind="0.0.0.0", port=8765):
    server = ThreadingHTTPServer((bind, port), SnapHandler)
    server.daemon_threads = True
    server.app_state = state
    return server
