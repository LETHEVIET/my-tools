"""GPUI Kit Quick Note window: shortcut/tray entry and Ctrl+S save + close.

Runs on a real Windows interactive session, never on headless CI. A dedicated
test mutex, user-data directory, and port isolate this process from the user's
installed copy. Keyboard injection is guarded by a foreground-window check.
"""
import ctypes
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import unittest

BIN = Path(os.environ.get("MY_TOOLS_AGENT_BIN") or
           (Path(__file__).resolve().parents[1] / "desktop" / "target" / "release" / "my-tools.exe"))


@unittest.skipIf(bool(os.environ.get("CI")), "GPUI requires an interactive desktop")
@unittest.skipUnless(os.name == "nt" and BIN.exists(), "Needs Windows My Tools release EXE")
class QuickNoteUiTest(unittest.TestCase):
    def test_ctrl_s_saves_note_and_closes_gpui_window(self):
        user32 = ctypes.WinDLL("user32", use_last_error=True)
        user32.EnumWindows.argtypes = [ctypes.c_void_p, ctypes.c_ssize_t]
        user32.EnumWindows.restype = ctypes.c_int
        user32.GetWindowTextW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]
        user32.GetWindowTextW.restype = ctypes.c_int
        user32.GetClassNameW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]
        user32.GetClassNameW.restype = ctypes.c_int
        user32.GetWindowThreadProcessId.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
        user32.GetWindowThreadProcessId.restype = ctypes.c_ulong
        user32.PostMessageW.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_size_t, ctypes.c_ssize_t]
        user32.PostMessageW.restype = ctypes.c_int
        user32.GetForegroundWindow.restype = ctypes.c_void_p
        user32.SetForegroundWindow.argtypes = [ctypes.c_void_p]
        user32.SetForegroundWindow.restype = ctypes.c_int
        user32.keybd_event.argtypes = [ctypes.c_ubyte, ctypes.c_ubyte, ctypes.c_uint, ctypes.c_size_t]
        CALLBACK = ctypes.WINFUNCTYPE(ctypes.c_int, ctypes.c_void_p, ctypes.c_ssize_t)

        def own_window(name=None, title=None):
            windows = []
            @CALLBACK
            def find(hwnd, _):
                pid = ctypes.c_ulong()
                user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
                if pid.value != proc.pid:
                    return 1
                value = ctypes.create_unicode_buffer(160)
                if name is not None:
                    user32.GetClassNameW(hwnd, value, len(value))
                    if value.value != name:
                        return 1
                if title is not None:
                    user32.GetWindowTextW(hwnd, value, len(value))
                    if value.value != title:
                        return 1
                windows.append(hwnd)
                return 0
            user32.EnumWindows(CALLBACK(find), 0)
            return windows[0] if windows else None

        def wait_for(check, seconds=14):
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                result = check()
                if result:
                    return result
                if proc.poll() is not None:
                    self.fail(f"My Tools exited unexpectedly: {proc.returncode}")
                time.sleep(0.1)
            self.fail("Timed out waiting for the Quick Note UI")

        with tempfile.TemporaryDirectory() as data:
            with socket.socket() as sock:
                sock.bind(("127.0.0.1", 0))
                port = sock.getsockname()[1]
            instance = f"note-{os.getpid()}-{port}"
            env = dict(os.environ, LOCALAPPDATA=data, MY_TOOLS_TEST_INSTANCE_ID=instance)
            proc = subprocess.Popen(
                [str(BIN), "--minimized", "--host", "127.0.0.1", "--port", str(port)],
                env=env,
            )
            try:
                tray = wait_for(lambda: own_window(name="MyToolsAgentWin32"))
                self.assertTrue(user32.PostMessageW(tray, 0x0111, 1010, 0))
                popup = wait_for(lambda: own_window(title="Quick Note - My Tools"))
                self.assertEqual(own_window(name="MyToolsPromptNotesPopup"), None,
                                 "The editor should use GPUI Kit, not Win32 EDIT")
                user32.SetForegroundWindow(popup)
                if user32.GetForegroundWindow() != popup:
                    self.skipTest("Windows did not permit focusing the isolated test popup")

                # Send only to the test popup, never to an unrelated app.
                content = "Quick note saves and closes"
                for letter in content:
                    self.assertTrue(user32.PostMessageW(popup, 0x0102, ord(letter), 1))

                draft = Path(data) / "MyTools" / "quick-note-draft.txt"
                wait_for(lambda: draft.exists() and content in draft.read_text(encoding="utf-8"),
                         seconds=5)
                if user32.GetForegroundWindow() != popup:
                    self.skipTest("Another application took foreground focus before Ctrl+S")

                user32.keybd_event(0x11, 0, 0, 0)  # Ctrl down
                user32.keybd_event(0x53, 0, 0, 0)  # S down
                user32.keybd_event(0x53, 0, 2, 0)  # S up
                user32.keybd_event(0x11, 0, 2, 0) # Ctrl up
                notes = Path(data) / "MyTools" / "quick-notes"
                wait_for(lambda: bool(list(notes.glob("note-*.txt"))), seconds=6)
                self.assertIsNone(own_window(title="Quick Note - My Tools"),
                                  "Ctrl+S must close the note popup after saving")
                self.assertEqual(len(list(notes.glob("note-*.txt"))), 1)
                self.assertEqual(next(notes.glob("note-*.txt")).read_text(encoding="utf-8"), content)
                self.assertEqual(draft.read_text(encoding="utf-8"), "")
                self.assertFalse((Path(data) / "MyTools" / "prompt-notes.txt").exists())
                self.assertIsNone(proc.poll())
                self.assertTrue(user32.PostMessageW(tray, 0x0111, 1005, 0))
                self.assertEqual(proc.wait(timeout=10), 0)
            finally:
                if proc.poll() is None:
                    proc.terminate()
                    proc.wait(timeout=5)
