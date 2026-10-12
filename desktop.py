"""Snap to Prompt Windows receiver and pairing UI."""
from __future__ import annotations

import argparse
from io import BytesIO
import queue
import socket
import sys
import threading
import tkinter as tk
from urllib.parse import urlsplit

import pystray
import qrcode
from PIL import Image, ImageDraw, ImageTk

from receiver_server import AppState, make_server
from windows_paste import perform_paste

BACKGROUND = "#101827"
CARD = "#1b293d"
TEXT = "#f4f7fc"
MUTED = "#9bacbf"
ACCENT = "#45dfb0"


def local_ip():
    # UDP connect doesn't send a packet. It only requests the system's route.
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
            sock.connect(("1.1.1.1", 80))
            return sock.getsockname()[0]
    except OSError:
        try:
            return socket.gethostbyname(socket.gethostname())
        except OSError:
            return "127.0.0.1"


class DesktopApp:
    def __init__(self, root: tk.Tk, host: str, port: int, public_url: str | None,
                 show_window: bool = False):
        self.root = root
        self.port = port
        self.host = host
        self.public_url = public_url
        self.messages = queue.Queue()
        self.closing = False
        self.auto_paste = tk.BooleanVar(value=True)
        self.state = AppState(self.on_photo)
        self.state.auto_paste = True
        self.server = make_server(self.state, port=port)
        threading.Thread(target=self.server.serve_forever, daemon=True, name="snap-http").start()
        self.render()
        self.hwnd = int(self.root.winfo_id())
        self.root.protocol("WM_DELETE_WINDOW", self.hide_window)
        self.tray_icon = self.make_tray_icon()
        if not show_window:
            self.root.withdraw()
        self.tray_thread = threading.Thread(target=self.run_tray, daemon=True, name="snap-tray")
        self.tray_thread.start()
        self.root.after(250, self.poll_messages)

    def base_url(self):
        return self.public_url.rstrip("/") if self.public_url else f"http://{self.host}:{self.port}"

    def pairing_url(self):
        # Token is a URL fragment (not sent in an HTTP request).
        return self.base_url() + "/#pair=" + self.state.token

    def render(self):
        self.root.title("Snap to Prompt — PC Receiver")
        self.root.geometry("494x655")
        self.root.minsize(480, 640)
        self.root.configure(bg=BACKGROUND)
        wrap = tk.Frame(self.root, bg=BACKGROUND, padx=28, pady=24)
        wrap.pack(fill="both", expand=True)
        tk.Label(wrap, text="◈  SNAP TO PROMPT", fg=ACCENT, bg=BACKGROUND,
                 font=("Segoe UI", 13, "bold")).pack(anchor="w")
        tk.Label(wrap, text="Snap on iPhone. Paste on Windows.", fg=TEXT, bg=BACKGROUND,
                 font=("Segoe UI", 17, "bold")).pack(anchor="w", pady=(13, 5))
        tk.Label(wrap, text="1  Scan this QR code on your iPhone.", fg=MUTED,
                 bg=BACKGROUND, font=("Segoe UI", 11)).pack(anchor="w", pady=(13, 10))
        self.qr_label = tk.Label(wrap, bd=0, bg="white", padx=14, pady=14)
        self.qr_label.pack()
        self.set_qr()
        self.url_label = tk.Label(wrap, text=self.base_url(), fg=ACCENT, bg=BACKGROUND,
                                  font=("Consolas", 10), cursor="hand2")
        self.url_label.pack(pady=(12, 4))
        self.url_label.bind("<Button-1>", lambda e: self.copy_link())
        tk.Label(wrap, text="2  Open ChatGPT / Claude and click into the prompt box.",
                 fg=MUTED, bg=BACKGROUND, font=("Segoe UI", 10)).pack(anchor="w", pady=(12, 7))
        self.toggle = tk.Checkbutton(wrap, text="Auto-paste into focused window",
                                     variable=self.auto_paste, command=self.toggle_auto,
                                     font=("Segoe UI", 11, "bold"), bg=CARD, fg=TEXT,
                                     activebackground=CARD, activeforeground=TEXT,
                                     selectcolor=CARD, padx=13, pady=11, anchor="w")
        self.toggle.pack(fill="x")
        self.status = tk.Label(wrap, text="●  Ready · 0 photos", fg=ACCENT,
                               bg=BACKGROUND, font=("Segoe UI", 10, "bold"))
        self.status.pack(anchor="w", pady=(15, 4))
        self.last_action = tk.Label(wrap, text="Close this window to hide it in the system tray.",
                                    fg=MUTED, bg=BACKGROUND, wraplength=400,
                                    justify="left", font=("Segoe UI", 9))
        self.last_action.pack(anchor="w")
        buttons = tk.Frame(wrap, bg=BACKGROUND)
        buttons.pack(fill="x", pady=(17, 0))
        self.make_button(buttons, "Copy pairing link", self.copy_link).pack(side="left", padx=(0, 8))
        self.make_button(buttons, "New QR code", self.reset_pairing).pack(side="left")
        self.make_button(buttons, "Hide to tray", self.hide_window).pack(side="left", padx=(8, 0))
        foot = "Local Wi-Fi · photos are not stored"
        if self.public_url:
            foot = "HTTPS tunnel active · only pair trusted devices"
        tk.Label(wrap, text=foot, fg=MUTED, bg=BACKGROUND,
                 font=("Segoe UI", 9)).pack(side="bottom", pady=(15, 0))

    def make_button(self, parent, text, cmd):
        return tk.Button(parent, text=text, command=cmd, bg=CARD, fg=TEXT,
                         activebackground="#2b415e", activeforeground="white", bd=0,
                         padx=15, pady=10, cursor="hand2", font=("Segoe UI", 9))

    def set_qr(self):
        qr = qrcode.QRCode(box_size=7, border=2,
                          error_correction=qrcode.constants.ERROR_CORRECT_M)
        qr.add_data(self.pairing_url())
        qr.make(fit=True)
        # Avoid GUI resize when changing the pairing code.
        image = qr.make_image(fill_color="#101827", back_color="white").convert("RGB")
        image.thumbnail((244, 244))
        self.qr_photo = ImageTk.PhotoImage(image)
        self.qr_label.configure(image=self.qr_photo)

    def copy_link(self):
        self.root.clipboard_clear()
        self.root.clipboard_append(self.pairing_url())
        self.last_action.configure(text="Pairing link copied to clipboard")

    def reset_pairing(self):
        self.state.rotate_token()
        self.set_qr()
        self.last_action.configure(text="New QR code created. Old pairing links won't work.")

    def toggle_auto(self):
        self.state.auto_paste = bool(self.auto_paste.get())
        self.last_action.configure(text="Auto-paste " + ("ON" if self.state.auto_paste else "OFF — clipboard only"))
        self.tray_icon.update_menu()

    @staticmethod
    def make_tray_image():
        # Tiny in-memory icon: no icon files, web renderer, or additional GUI framework.
        image = Image.new("RGBA", (64, 64), (16, 24, 39, 255))
        draw = ImageDraw.Draw(image)
        draw.rounded_rectangle((17, 10, 47, 54), radius=7, outline=(69, 223, 176), width=5)
        draw.ellipse((25, 25, 39, 39), fill=(69, 223, 176))
        return image

    def make_tray_icon(self):
        def command(name):
            # Tray callbacks run off the Tk thread; dispatch back to Tk's event loop.
            return lambda icon, item: self.messages.put(("command", name))

        return pystray.Icon(
            "SnapToPrompt", self.make_tray_image(), "Snap to Prompt",
            pystray.Menu(
                pystray.MenuItem("Show pairing QR", command("show"), default=True),
                pystray.MenuItem("Auto-paste", command("toggle"),
                                 checked=lambda item: self.state.auto_paste),
                pystray.MenuItem("New pairing QR", command("pair")),
                pystray.Menu.SEPARATOR,
                pystray.MenuItem("Exit Snap to Prompt", command("exit")),
            ),
        )

    def run_tray(self):
        try:
            self.tray_icon.run()
        except Exception as exc:
            self.messages.put(("tray_error", str(exc)))

    def show_window(self):
        self.root.deiconify()
        self.root.lift()

    def hide_window(self):
        self.root.withdraw()

    def tray_command(self, name):
        if name == "show":
            self.show_window()
        elif name == "toggle":
            self.auto_paste.set(not self.auto_paste.get())
            self.toggle_auto()
        elif name == "pair":
            self.reset_pairing()
            self.show_window()
        elif name == "exit":
            self.close()

    def on_photo(self, image):
        """Invoked in a server thread; paste immediately, report via main-thread queue."""
        try:
            outcome = perform_paste(image, self.hwnd, self.state.auto_paste)
            self.messages.put((True, outcome))
            return outcome
        except Exception as exc:
            self.messages.put((False, str(exc)))
            raise

    def poll_messages(self):
        if self.closing:
            return
        try:
            while True:
                kind, msg = self.messages.get_nowait()
                if kind == "command":
                    self.tray_command(msg)
                    if self.closing:
                        return
                    continue
                if kind == "tray_error":
                    self.show_window()
                    self.last_action.configure(text="Tray unavailable: " + msg)
                    continue
                ok = kind
                self.status.configure(text=("●  Ready" if ok else "●  Attention") + f" · {self.state.received} photos",
                                      fg=ACCENT if ok else "#ffb86c")
                self.last_action.configure(text=msg)
        except queue.Empty:
            pass
        self.root.after(250, self.poll_messages)

    def close(self):
        if self.closing:
            return
        self.closing = True
        self.tray_icon.stop()
        self.server.shutdown()
        self.server.server_close()
        self.root.destroy()


def main():
    if sys.platform != "win32":
        print("The desktop helper runs on Windows. Run tests on other platforms using pytest.")
        return 1
    parser = argparse.ArgumentParser(description="Snap to Prompt Windows receiver")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--host", default=None, help="PC's Wi-Fi IPv4 to show in the QR code")
    parser.add_argument("--public-url", default=None, help="Optional trusted HTTPS tunnel URL")
    parser.add_argument("--show-window", action="store_true", help="Show the pairing window on startup")
    args = parser.parse_args()
    if args.public_url and urlsplit(args.public_url).scheme != "https":
        parser.error("--public-url must be HTTPS")
    root = tk.Tk()
    try:
        app = DesktopApp(root, args.host or local_ip(), args.port, args.public_url, args.show_window)
    except OSError as exc:
        print(f"Cannot open port {args.port}: {exc}", file=sys.stderr)
        return 1
    root.mainloop()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
