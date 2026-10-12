"""One-process My Tools lifecycle: desktop, hide-to-tray, reopen, exit."""
import ctypes
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import unittest
from urllib.request import urlopen


BIN = Path(os.environ.get("MY_TOOLS_AGENT_BIN") or
           (Path(__file__).resolve().parents[1] / "desktop" / "target" / "release" / "my-tools.exe"))


@unittest.skipIf(bool(os.environ.get("CI")), "GPUI lifecycle requires an interactive Windows desktop")
@unittest.skipUnless(os.name == "nt" and BIN.exists(), "Needs the built Windows My Tools EXE")
class SingleAppTrayTest(unittest.TestCase):
    def test_dashboard_hides_without_stopping_agent(self):
        user32 = ctypes.WinDLL("user32", use_last_error=True)
        user32.FindWindowW.argtypes = [ctypes.c_wchar_p, ctypes.c_wchar_p]
        user32.FindWindowW.restype = ctypes.c_void_p
        user32.IsWindowVisible.argtypes = [ctypes.c_void_p]
        user32.IsWindowVisible.restype = ctypes.c_int
        user32.PostMessageW.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_size_t, ctypes.c_ssize_t]
        user32.PostMessageW.restype = ctypes.c_int
        user32.GetWindowThreadProcessId.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
        user32.GetWindowThreadProcessId.restype = ctypes.c_ulong
        user32.GetClassNameW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]
        user32.GetClassNameW.restype = ctypes.c_int
        CALLBACK = ctypes.WINFUNCTYPE(ctypes.c_int, ctypes.c_void_p, ctypes.c_ssize_t)
        user32.EnumWindows.argtypes = [CALLBACK, ctypes.c_ssize_t]
        user32.EnumWindows.restype = ctypes.c_int

        def owned_window(class_name, title, pid):
            found = []
            @CALLBACK
            def check(hwnd, _):
                window_pid = ctypes.c_ulong()
                user32.GetWindowThreadProcessId(hwnd, ctypes.byref(window_pid))
                if window_pid.value != pid:
                    return 1
                if class_name is not None:
                    name = ctypes.create_unicode_buffer(120)
                    user32.GetClassNameW(hwnd, name, len(name))
                    if name.value != class_name:
                        return 1
                if title is not None:
                    name = ctypes.create_unicode_buffer(120)
                    user32.GetWindowTextW(hwnd, name, len(name))
                    if name.value != title:
                        return 1
                found.append(hwnd)
                return 0
            user32.EnumWindows(check, 0)
            return found[0] if found else None

        def until(predicate, seconds=16):
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                found = predicate()
                if found:
                    return found
                if proc.poll() is not None:
                    self.fail(f"My Tools exited unexpectedly with status {proc.returncode}")
                time.sleep(0.10)
            self.fail("My Tools desktop/tray lifecycle did not reach the expected state")

        with tempfile.TemporaryDirectory() as tmp:
            with socket.socket() as sock:
                sock.bind(("127.0.0.1", 0))
                port = sock.getsockname()[1]
            instance = f"tray-{os.getpid()}-{port}"
            env = dict(os.environ, LOCALAPPDATA=tmp, MY_TOOLS_TEST_INSTANCE_ID=instance)
            proc = subprocess.Popen([str(BIN), "--minimized", "--host", "127.0.0.1",
                                     "--port", str(port)], env=env)
            try:
                tray = until(lambda: owned_window("MyToolsAgentWin32", None, proc.pid))
                hub = until(lambda: owned_window(None, f"My Tools [test-{instance}]", proc.pid))
                self.assertFalse(user32.IsWindowVisible(hub))

                # The same tray process shows the GPUI window.
                self.assertTrue(user32.PostMessageW(tray, 0x0111, 1009, 0))  # WM_COMMAND / Show hub
                until(lambda: user32.IsWindowVisible(hub))

                # Windows close button must hide rather than terminate.
                self.assertTrue(user32.PostMessageW(hub, 0x0010, 0, 0))  # WM_CLOSE
                until(lambda: not user32.IsWindowVisible(hub))
                self.assertIsNone(proc.poll())
                with urlopen(f"http://127.0.0.1:{port}/", timeout=3) as response:
                    self.assertEqual(response.status, 200)

                # Relaunching the EXE should reveal the already-running app.
                duplicate = subprocess.Popen([str(BIN)], env=env)
                self.assertEqual(duplicate.wait(timeout=8), 0)
                until(lambda: user32.IsWindowVisible(hub))

                # Pairing is displayed in the existing GPUI dashboard, not
                # a browser. The tray command must reveal the hidden hub.
                self.assertTrue(user32.PostMessageW(hub, 0x0010, 0, 0))
                until(lambda: not user32.IsWindowVisible(hub))
                self.assertTrue(user32.PostMessageW(tray, 0x0111, 1001, 0))  # Show pairing QR
                until(lambda: user32.IsWindowVisible(hub))
                self.assertIsNone(proc.poll())

                # Global Quick Launcher is also accessible through the tray.
                self.assertTrue(user32.PostMessageW(tray, 0x0111, 1011, 0))
                palette = until(lambda: owned_window(None, "Quick Launcher - My Tools", proc.pid))
                self.assertTrue(user32.IsWindowVisible(palette))
                self.assertTrue(user32.PostMessageW(palette, 0x0010, 0, 0))
                until(lambda: not owned_window(None, "Quick Launcher - My Tools", proc.pid))

                # Quick Tasks navigation reuses the existing GPUI dashboard.
                self.assertTrue(user32.PostMessageW(tray, 0x0111, 1012, 0))
                until(lambda: user32.IsWindowVisible(hub))
                self.assertIsNone(proc.poll())

                # True exit cleans up the tray and exits the one process.
                self.assertTrue(user32.PostMessageW(tray, 0x0111, 1005, 0))  # WM_COMMAND / Exit
                self.assertEqual(proc.wait(timeout=10), 0)
            finally:
                if proc.poll() is None:
                    proc.terminate()
                    proc.wait(timeout=5)
