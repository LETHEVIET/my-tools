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


BIN = Path(os.environ.get("MY_TOOLS_AGENT_BIN") or
           (Path(__file__).resolve().parents[1] / "desktop" / "target" / "release" / "my-tools.exe"))


@unittest.skipUnless(os.name == "nt" and BIN.exists(), "Requires built Windows Rust executable")
class RustReceiverTests(unittest.TestCase):
    def test_local_pair_status_and_invalid_upload(self):
        with tempfile.TemporaryDirectory() as directory:
            secret = "t" * 43
            # Migrate an existing user's saved pairing rather than forcing a new QR scan.
            old_token_file = Path(directory) / "SnapToPrompt" / "pairing-token.txt"
            old_token_file.parent.mkdir()
            old_token_file.write_text(secret + "\n", encoding="ascii")
            token_file = Path(directory) / "MyTools" / "pairing-token.txt"
            token_file.parent.mkdir()
            token_file.write_text("incomplete-migrated-pairing\n", encoding="ascii")
            with socket.socket() as s:
                s.bind(("127.0.0.1", 0))
                port = s.getsockname()[1]
            env = dict(os.environ, LOCALAPPDATA=directory)
            process = subprocess.Popen([str(BIN), "--headless-test", "--host", "127.0.0.1", "--port", str(port)], env=env)

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
                self.assertIn(b"My Tools", raw)
                status, tools = request("/api/tools")
                self.assertEqual(status, 200)
                self.assertEqual(json.loads(tools)["tools"][0]["id"], "snap-to-paste-image")
                self.assertEqual(json.loads(tools)["tools"][1]["id"], "prompt-notes")
                self.assertEqual(json.loads(tools)["tools"][2]["id"], "quick-note")
                status, tool = request("/tools/snap-to-paste-image/")
                self.assertEqual(status, 200)
                self.assertIn(b"libraryInput", tool)
                self.assertIn(b"nativeCameraInput", tool)
                self.assertEqual(request("/api/status")[0], 401)
                self.assertEqual(request("/api/status", secret)[0], 200)
                self.assertEqual(request("/api/tools/snap-to-paste-image/status", secret)[0], 200)
                self.assertIn(b"<svg", request("/pair")[1])
                self.assertIn(b"/tools/snap-to-paste-image/#pair=", request("/pair")[1])
                self.assertEqual(request("/api/photo", secret, b"bad-image")[0], 415)
                self.assertEqual(request("/api/tools/snap-to-paste-image/photo", secret, b"bad-image")[0], 415)
                self.assertEqual(json.loads(request("/api/status", secret)[1])["received"], 0)
                self.assertEqual(token_file.read_text(encoding="ascii").strip(), secret)
                self.assertEqual(old_token_file.read_text(encoding="ascii").strip(), secret)
            finally:
                process.terminate()
                process.wait(timeout=5)


if __name__ == "__main__":
    unittest.main()
