"""Windows-only regression test for Quick Note's save-only workflow."""
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


@unittest.skipIf(bool(os.environ.get("CI")), "Win32 popover test needs an interactive Windows desktop")
@unittest.skipUnless(os.name == "nt" and BIN.exists(), "Needs the built Windows My Tools EXE")
class QuickNoteUiTest(unittest.TestCase):
    def test_save_note_does_not_paste_and_can_reopen(self):
        user32 = ctypes.WinDLL("user32", use_last_error=True)
        user32.FindWindowW.argtypes = [ctypes.c_wchar_p, ctypes.c_wchar_p]
        user32.FindWindowW.restype = ctypes.c_void_p
        user32.PostMessageW.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_size_t, ctypes.c_ssize_t]
        user32.PostMessageW.restype = ctypes.c_int
        user32.GetDlgItem.argtypes = [ctypes.c_void_p, ctypes.c_int]
        user32.GetDlgItem.restype = ctypes.c_void_p
        user32.SetWindowTextW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p]
        user32.SetWindowTextW.restype = ctypes.c_int
        user32.GetWindowTextW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]
        user32.GetWindowTextW.restype = ctypes.c_int
        user32.IsWindowVisible.argtypes = [ctypes.c_void_p]
        user32.IsWindowVisible.restype = ctypes.c_int
        user32.SendMessageW.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_size_t, ctypes.c_ssize_t]
        user32.SendMessageW.restype = ctypes.c_ssize_t
        user32.GetWindowThreadProcessId.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_ulong)]
        user32.GetWindowThreadProcessId.restype = ctypes.c_ulong
        user32.GetClassNameW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_int]
        user32.GetClassNameW.restype = ctypes.c_int
        CALLBACK = ctypes.WINFUNCTYPE(ctypes.c_int, ctypes.c_void_p, ctypes.c_ssize_t)
        user32.EnumWindows.argtypes = [CALLBACK, ctypes.c_ssize_t]
        user32.EnumWindows.restype = ctypes.c_int

        def own_window(class_name):
            found = []
            @CALLBACK
            def check(hwnd, _):
                pid = ctypes.c_ulong()
                user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
                name = ctypes.create_unicode_buffer(120)
                user32.GetClassNameW(hwnd, name, len(name))
                if pid.value == proc.pid and name.value == class_name:
                    found.append(hwnd)
                    return 0
                return 1
            user32.EnumWindows(check, 0)
            return found[0] if found else None

        def wait_for(callback, seconds=15):
            deadline = time.monotonic() + seconds
            while time.monotonic() < deadline:
                value = callback()
                if value:
                    return value
                if proc.poll() is not None:
                    self.fail(f"My Tools exited early: {proc.returncode}")
                time.sleep(0.1)
            self.fail("Quick Note window did not reach the expected state")

        with tempfile.TemporaryDirectory() as data:
            with socket.socket() as sock:
                sock.bind(("127.0.0.1", 0))
                port = sock.getsockname()[1]
            instance = f"quick-{os.getpid()}-{port}"
            env = dict(os.environ, LOCALAPPDATA=data, MY_TOOLS_TEST_INSTANCE_ID=instance)
            proc = subprocess.Popen([str(BIN), "--minimized", "--host", "127.0.0.1",
                                     "--port", str(port)], env=env)
            try:
                tray = wait_for(lambda: own_window("MyToolsAgentWin32"))
                self.assertTrue(user32.PostMessageW(tray, 0x0111, 1010, 0))  # Quick Note tray command
                popup = wait_for(lambda: own_window("MyToolsPromptNotesPopup"))
                self.assertTrue(wait_for(lambda: user32.IsWindowVisible(popup)))
                edit = user32.GetDlgItem(popup, 3101)
                self.assertTrue(edit)
                content = "My shopping list\nMilk and coffee 你好"
                # SetWindowTextW cannot edit controls belonging to another
                # process; WM_SETTEXT sends the text to the actual EDIT widget.
                text_buffer = ctypes.create_unicode_buffer(content)
                self.assertTrue(user32.SendMessageW(edit, 0x000C, 0, ctypes.cast(text_buffer, ctypes.c_void_p).value))
                self.assertTrue(user32.PostMessageW(popup, 0x0111, 3102, 0))  # Save Note button
                notes_dir = Path(data) / "MyTools" / "quick-notes"
                try:
                    wait_for(lambda: not user32.IsWindowVisible(popup), seconds=4)
                except AssertionError as failure:
                    title = ctypes.create_unicode_buffer(120)
                    user32.GetWindowTextW(popup, title, len(title))
                    files = list(notes_dir.glob('*.txt')) if notes_dir.exists() else []
                    other_windows = []
                    dialog_texts = []
                    user32.EnumChildWindows.argtypes = [ctypes.c_void_p, CALLBACK, ctypes.c_ssize_t]
                    user32.EnumChildWindows.restype = ctypes.c_int
                    @CALLBACK
                    def describe(hwnd, _):
                        pid = ctypes.c_ulong()
                        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
                        if pid.value == proc.pid:
                            text = ctypes.create_unicode_buffer(120)
                            user32.GetWindowTextW(hwnd, text, len(text))
                            other_windows.append(text.value)
                            if text.value == 'Prompt Notes':
                                @CALLBACK
                                def children(child, _):
                                    contents = ctypes.create_unicode_buffer(1000)
                                    user32.GetWindowTextW(child, contents, len(contents))
                                    dialog_texts.append(contents.value)
                                    return 1
                                user32.EnumChildWindows(hwnd, children, 0)
                        return 1
                    user32.EnumWindows(describe, 0)
                    raise AssertionError(f"{failure}: title={title.value!r}, note_files={len(files)}, contents={[x.name for x in (Path(data)/'MyTools').rglob('*')]}, windows={other_windows}, dialog={dialog_texts}")
                note_files = list(notes_dir.glob("note-*.txt"))
                self.assertEqual(len(note_files), 1)
                self.assertEqual(note_files[0].read_text(encoding="utf-8"), content)
                self.assertFalse((Path(data) / "MyTools" / "prompt-notes.txt").exists())

                self.assertTrue(user32.PostMessageW(tray, 0x0111, 1010, 0))
                wait_for(lambda: user32.IsWindowVisible(popup))
                saved = user32.GetDlgItem(popup, 3107)
                self.assertTrue(saved)
                # Select the most recent note and simulate the native double-click event.
                user32.SendMessageW(saved, 0x0186, 0, 0)  # LB_SETCURSEL
                self.assertTrue(user32.PostMessageW(popup, 0x0111, (2 << 16) | 3107, 0))
                def read_edit():
                    buf = ctypes.create_unicode_buffer(500)
                    user32.SendMessageW(edit, 0x000D, 500, ctypes.cast(buf, ctypes.c_void_p).value)
                    return buf.value
                wait_for(lambda: read_edit() == content)
                self.assertIsNone(proc.poll())
                self.assertTrue(user32.PostMessageW(tray, 0x0111, 1005, 0))
                self.assertEqual(proc.wait(timeout=10), 0)
            finally:
                if proc.poll() is None:
                    proc.terminate()
                    proc.wait(timeout=5)
