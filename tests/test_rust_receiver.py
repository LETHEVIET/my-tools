"""Safe Windows integration test: no valid images and therefore no Ctrl+V injection.

Run after `cargo build --release --manifest-path rust/Cargo.toml`.
"""
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import unittest
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


BIN = Path(os.environ.get("SNAP_RUST_BIN") or
           (Path(__file__).resolve().parents[1] / "rust" / "target" / "release" / "snap-to-prompt.exe"))


@unittest.skipUnless(os.name == "nt" and BIN.exists(), "Requires built Windows Rust executable")
class RustReceiverTests(unittest.TestCase):
    def test_local_pair_status_and_invalid_upload(self):
        with tempfile.TemporaryDirectory() as directory:
            secret = "t" * 43
            token_file = Path(directory) / "SnapToPrompt" / "pairing-token.txt"
            token_file.parent.mkdir()
            token_file.write_text(secret + "\n", encoding="ascii")
            with socket.socket() as s:
                s.bind(("127.0.0.1", 0))
                port = s.getsockname()[1]
            env = dict(os.environ, LOCALAPPDATA=directory)
            process = subprocess.Popen([str(BIN), "--host", "127.0.0.1", "--port", str(port)], env=env)

            def request(path, token=None, data=None):
                headers = {"Authorization": f"Bearer {token}"} if token is not None else {}
                req = Request(f"http://127.0.0.1:{port}{path}", data=data, headers=headers,
                              method="POST" if data is not None else "GET")
                try:
                    with urlopen(req, timeout=3) as response:
                        return response.status, response.read()
                except HTTPError as error:
                    with error:
                        return error.code, error.read()

            try:
                for _ in range(60):
                    try:
                        status, raw = request("/")
                        break
                    except (ConnectionError, URLError, TimeoutError):
                        if process.poll() is not None:
                            self.fail(f"Rust receiver exited early: {process.returncode}")
                        time.sleep(0.1)
                else:
                    self.fail("Rust receiver did not start in six seconds")
                self.assertEqual(status, 200)
                self.assertIn(b"libraryInput", raw)
                self.assertIn(b"nativeCameraInput", raw)
                self.assertEqual(request("/api/status")[0], 401)
                self.assertEqual(request("/api/status", secret)[0], 200)
                self.assertIn(b"<svg", request("/pair")[1])
                self.assertEqual(request("/api/photo", secret, b"bad-image")[0], 415)
                self.assertEqual(json.loads(request("/api/status", secret)[1])["received"], 0)
                self.assertEqual(token_file.read_text(encoding="ascii").strip(), secret)
            finally:
                process.terminate()
                process.wait(timeout=5)


if __name__ == "__main__":
    unittest.main()
