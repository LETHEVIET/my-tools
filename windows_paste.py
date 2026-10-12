"""Win32 image clipboard and keyboard injection; standard library ctypes only."""
from __future__ import annotations

import ctypes
from ctypes import wintypes
import sys
import time
from PIL import Image
from image_processing import image_to_dib


class PasteError(RuntimeError):
    pass


def _require_windows() -> None:
    if sys.platform != "win32":
        raise PasteError("Native clipboard and Ctrl+V are available on Windows only")


def copy_image_to_clipboard(image: Image.Image, owner_hwnd: int) -> None:
    _require_windows()
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    HANDLE = wintypes.HANDLE
    user32.OpenClipboard.argtypes = [wintypes.HWND]
    user32.OpenClipboard.restype = wintypes.BOOL
    user32.EmptyClipboard.argtypes = []
    user32.EmptyClipboard.restype = wintypes.BOOL
    user32.SetClipboardData.argtypes = [wintypes.UINT, HANDLE]
    user32.SetClipboardData.restype = HANDLE
    user32.CloseClipboard.argtypes = []
    user32.CloseClipboard.restype = wintypes.BOOL
    kernel32.GlobalAlloc.argtypes = [wintypes.UINT, ctypes.c_size_t]
    kernel32.GlobalAlloc.restype = HANDLE
    kernel32.GlobalLock.argtypes = [HANDLE]
    kernel32.GlobalLock.restype = ctypes.c_void_p
    kernel32.GlobalUnlock.argtypes = [HANDLE]
    kernel32.GlobalUnlock.restype = wintypes.BOOL
    kernel32.GlobalFree.argtypes = [HANDLE]
    kernel32.GlobalFree.restype = HANDLE

    dib = image_to_dib(image)
    hmem = kernel32.GlobalAlloc(0x0002, len(dib))  # GMEM_MOVEABLE
    if not hmem:
        raise PasteError("Could not allocate clipboard memory")
    ptr = kernel32.GlobalLock(hmem)
    if not ptr:
        kernel32.GlobalFree(hmem)
        raise PasteError("Could not lock clipboard memory")
    try:
        ctypes.memmove(ptr, dib, len(dib))
    finally:
        kernel32.GlobalUnlock(hmem)

    opened = False
    for _ in range(15):
        if user32.OpenClipboard(owner_hwnd):
            opened = True
            break
        time.sleep(0.05)
    if not opened:
        kernel32.GlobalFree(hmem)
        raise PasteError("Clipboard busy; try again")
    try:
        if not user32.EmptyClipboard():
            raise PasteError("Could not clear clipboard")
        if not user32.SetClipboardData(8, hmem):  # CF_DIB; OS owns hmem after success
            raise PasteError("Could not write image to clipboard")
        hmem = None
    finally:
        user32.CloseClipboard()
        if hmem:
            kernel32.GlobalFree(hmem)


def paste_ctrl_v(owner_hwnd: int) -> None:
    """Send Ctrl+V to existing foreground window without stealing focus."""
    _require_windows()
    user32 = ctypes.WinDLL("user32", use_last_error=True)
    user32.GetForegroundWindow.restype = wintypes.HWND
    user32.GetAncestor.argtypes = [wintypes.HWND, wintypes.UINT]
    user32.GetAncestor.restype = wintypes.HWND
    foreground = user32.GetForegroundWindow()
    if not foreground:
        raise PasteError("No focused window. Image was copied to clipboard")
    # GA_ROOT: avoid typing into Snap to Prompt's own Tk window.
    if owner_hwnd and user32.GetAncestor(foreground, 2) == user32.GetAncestor(owner_hwnd, 2):
        raise PasteError("Focus your AI prompt on the PC. Image was copied to clipboard")

    ULONG_PTR = ctypes.c_ulonglong if ctypes.sizeof(ctypes.c_void_p) == 8 else ctypes.c_ulong

    class KEYBDINPUT(ctypes.Structure):
        _fields_ = [("wVk", wintypes.WORD), ("wScan", wintypes.WORD),
                    ("dwFlags", wintypes.DWORD), ("time", wintypes.DWORD),
                    ("dwExtraInfo", ULONG_PTR)]

    class MOUSEINPUT(ctypes.Structure):
        _fields_ = [("dx", wintypes.LONG), ("dy", wintypes.LONG),
                    ("mouseData", wintypes.DWORD), ("dwFlags", wintypes.DWORD),
                    ("time", wintypes.DWORD), ("dwExtraInfo", ULONG_PTR)]

    class HARDWAREINPUT(ctypes.Structure):
        _fields_ = [("uMsg", wintypes.DWORD), ("wParamL", wintypes.WORD),
                    ("wParamH", wintypes.WORD)]

    class INPUTUNION(ctypes.Union):
        _fields_ = [("ki", KEYBDINPUT), ("mi", MOUSEINPUT), ("hi", HARDWAREINPUT)]

    class INPUT(ctypes.Structure):
        _fields_ = [("type", wintypes.DWORD), ("u", INPUTUNION)]

    def key(vkey: int, up: bool = False) -> INPUT:
        return INPUT(type=1, u=INPUTUNION(ki=KEYBDINPUT(vkey, 0, 0x0002 if up else 0, 0, 0)))

    inputs = (INPUT * 4)(key(0x11), key(0x56), key(0x56, True), key(0x11, True))
    user32.SendInput.argtypes = [wintypes.UINT, ctypes.POINTER(INPUT), ctypes.c_int]
    user32.SendInput.restype = wintypes.UINT
    sent = user32.SendInput(4, inputs, ctypes.sizeof(INPUT))
    if sent != 4:
        raise PasteError("Ctrl+V could not be injected (try a non-administrator app)")


def perform_paste(image: Image.Image, owner_hwnd: int, auto_paste: bool) -> str:
    copy_image_to_clipboard(image, owner_hwnd)
    if not auto_paste:
        return "Photo copied to clipboard (auto-paste is off)"
    time.sleep(0.12)
    paste_ctrl_v(owner_hwnd)
    return "Photo pasted into the focused PC window"
