# Historical Snap to Prompt / Python prototype documentation

**Installable Windows releases and automatic updates:** see [UPDATE_GUIDE.md](UPDATE_GUIDE.md). Source: https://github.com/LETHEVIET/my-tools (private).

**My Tools** is the application name (desktop `my-tools.exe`, background `my-tools-agent.exe`). **Snap to Paste Image** and **Prompt Notes** (Ctrl+Alt+P to compose longer prompts, Ctrl+Enter to paste) are its first two tools. The Windows desktop hub uses **GPUI Kit with its default theme**; the native Rust tray receiver stays separate and lightweight. See [MY_TOOLS.md](MY_TOOLS.md) for the new structure, building the GPUI hub, using the shortcut scratchpad, app-name migration, and adding more tools. The mobile landing page is `/`, and the image tool is `/tools/snap-to-paste-image/`.

The original Snap to Prompt docs are retained below for historical reference.

**Current receiver: native Rust system-tray app.** See [RUST_SETUP.md](RUST_SETUP.md) for the new build, launch, pairing, enhanced iPhone Photo Library and in-app HTTPS camera, and testing instructions. `start_windows.bat` now launches Rust; `build_windows.bat` builds Rust. The remainder of this README documents the **older Python MVP**, preserved for rollback via `start_python_legacy.bat`.

An iPhone camera page paired with a Windows desktop receiver. Take a picture and, after accepting the photo in the iPhone camera UI, the PC receives the image, puts it on the Windows clipboard, and sends **Ctrl+V to the foreground window**. Works with ChatGPT, Claude, and other apps **that accept image paste**. It does **not** press Enter / send your prompt.

This is a runnable **MVP source project**, not a prebuilt Windows binary. The server and upload tests pass in the development environment; native Windows clipboard and keystroke injection require testing on a Windows PC.

## Quick start (same Wi-Fi, simplest)

1. On your **Windows PC**, install **Python 3.10 or newer** from https://www.python.org/downloads/windows/ (enable *Add Python to PATH*).
2. Extract the project folder. Double-click **`start_windows.bat`**. The first run installs Pillow and qrcode into a local `.venv`. Keep the receiver window running.
3. If Windows Firewall asks, **allow access on private networks only**. Connect the iPhone and PC to the **same Wi-Fi**. Avoid guest Wi-Fi/client-isolation networks.
4. Scan the QR code shown by the receiver **using the iPhone Camera app** and open the resulting page in Safari. The QR link securely pairs the session using a random token.
5. On Windows, open ChatGPT, Claude, etc., and **click into the prompt text box** so it has keyboard focus. The receiver GUI must *not* be the active window.
6. On the iPhone page, tap **Take Photo & Paste**, take a photo, then accept/**Use Photo** in the iPhone camera UI. The image is uploaded and automatically pasted into the focused PC app. The iPhone displays the result.
7. Keep the Windows helper running in the background; you can minimize it. For convenience, bookmark the paired Safari page on the iPhone.

**Tip:** You can type part of a prompt on PC, then snap a photo, and continue typing. Don't click anything else on PC while the photo uploads.

**No QR camera scanning on PC is required:** the PC *shows* a QR; the iPhone scans it once. If you press **New QR code** in the Windows app, old pairings are invalidated.

## Why the local demo isn't a fully installable PWA

This first version serves the camera page from `http://<LAN-IP>:8765`. iOS's **native HTML file capture** is used for still photos and should work without `getUserMedia` or HTTPS. This does not mean the page is a complete, secure-context PWA: service workers and a continuous in-page camera preview require HTTPS. The project *already contains* its PWA manifest, icons, and service worker, which activate on a proper HTTPS origin.

### Optional HTTPS / installable PWA mode

For development, you can use a trusted HTTPS reverse tunnel. One option is Cloudflare's `cloudflared` quick tunnel, but **photos will transit Cloudflare** rather than staying local.

1. Install `cloudflared` on Windows using its official instructions: https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/do-more-with-tunnels/trycloudflare/
2. In a **separate terminal**, run `cloudflared tunnel --url http://127.0.0.1:8765`. Note the `https://....trycloudflare.com` URL it prints. (It can run before the receiver starts.)
3. Launch the receiver using `start_windows.bat --public-url https://YOUR-TUNNEL.trycloudflare.com` (or relaunch if already running). It will show an HTTPS pairing QR.
4. Scan that QR on iPhone. If offered, use Safari's **Share → Add to Home Screen**. In the HTTPS app, you can also select **Use live camera** for in-page camera access, then tap **Snap & Paste**.

**Important:** Quick tunnel URLs are temporary. Re-pair when the URL changes. The service worker caches only static UI files; it does not store photos. While offline, the PWA shell may open but photo sending will fail. For production, use a stable HTTPS hostname and a properly secured transport.

## What runs where

| Piece | Technology | Purpose |
|---|---|---|
| iPhone camera page | HTML / CSS / JavaScript + PWA manifest & service worker | Native camera file capture (LAN), optional live camera (HTTPS), and photo upload |
| Pairing | 256-bit cryptographically random bearer token in QR URL fragment | Authorize a single Windows receiver session |
| Transfer | HTTP POST over local Wi-Fi; optional HTTPS reverse tunnel | Deliver a still image to the PC |
| Windows receiver | Python stdlib `ThreadingHTTPServer`, Tkinter GUI | Display QR and auto-paste toggle |
| Clipboard | Win32 `SetClipboardData` using `CF_DIB` | Put image pixels (not a file path or link) on Windows clipboard |
| Auto-paste | Win32 `SendInput` | Send Ctrl+V to the current foreground window |

## Build a standalone Windows executable

On Windows, double-click **`build_windows.bat`**. It installs PyInstaller and builds **`dist\SnapToPrompt.exe`** (one-file bundle with the web assets). This build is not performed in this environment and must be verified on Windows.

## Privacy and security considerations

- **Photos are processed in memory** and are **not written to disk** by the receiver. The last preview stays in iPhone browser memory until you leave/reload the page.
- The QR contains a random secret token in the **URL fragment**. The iPhone sends it as an `Authorization` header. The receiver rejects missing/expired tokens. Do not share the QR with strangers.
- **LAN HTTP is unencrypted**. Use only on a trusted private Wi-Fi network; don't expose TCP port 8765 to the public internet. For encrypted transport use HTTPS as described above.
- The helper **can overwrite your clipboard** and paste into the **current active PC window**. It cannot reliably determine if a text box is focused. Leave your AI prompt focused, or turn off **Auto-paste into focused window** in the receiver. With auto-paste off, images are copied to clipboard for manual pasting.
- The helper refuses to auto-paste into its *own* foreground window, but doesn't try to identify arbitrary other apps. If Windows is locked, or your target is elevated, native paste may fail. You may still have the image on the clipboard.
- Only upload photos you want copied to your PC. An optional public HTTPS tunnel is a third-party intermediary; do not use it for sensitive images unless its privacy model is acceptable to you.
- For production: add paired-device management, request origin enforcement, logged-in per-device sessions, a stable HTTPS endpoint, and stronger target-window restrictions.

## Troubleshooting

| Problem | Try |
|---|---|
| QR page doesn't load | Check both devices' Wi-Fi; allow private-network firewall access; guest Wi-Fi isolation; ensure receiver is open. You can run `start_windows.bat --host 192.168.1.123` if the PC has multiple network adapters. |
| Photo uploads but doesn't paste | Click directly into an AI prompt on the PC, not the receiver. Confirm auto-paste switch is on. Some programs don't accept pasted bitmap images. |
| Image in clipboard but no Ctrl+V | Windows may block injection into administrator/elevated apps, or no foreground window is available. |
| Unsupported or damaged image | Try taking a fresh photo instead of selecting a HEIC file from the library; JPEG and PNG are supported by default. |
| Wrong/stale pairing | On PC press **New QR code**, then rescan from iPhone. |
| Live camera button missing | LAN HTTP isn't HTTPS. Use native Take Photo; or configure optional HTTPS. |
| Works once, then fails | Check Windows firewall, PC sleep, Wi-Fi IP changes, and the temporary HTTPS tunnel hostname (if using one). |

## Run the automated tests

From the project folder on any machine with Pillow installed:

```sh
python -m unittest discover -s tests -v
```

Tests exercise valid photo upload, rejection of corrupt images, pairing, token rotation, static PWA files, image normalization, and DIB encoding. Native Windows Ctrl+V must be tested on the actual PC.

## Project structure

```text
snap-to-prompt/
├─ desktop.py              Windows Tkinter receiver app
├─ receiver_server.py      Pairing, HTTP server, upload validation
├─ image_processing.py     JPEG/PNG decode, orientation, bitmap encoding
├─ windows_paste.py        Native Windows clipboard + SendInput
├─ web/
│  ├─ index.html           iPhone camera screen
│  ├─ app.css              Mobile-first styling
│  ├─ app.js               Pairing, photo capture/upload, feedback
│  ├─ manifest.webmanifest
│  ├─ sw.js
│  └─ icons/
├─ tests/
├─ requirements.txt
├─ start_windows.bat
└─ build_windows.bat
```

## Version 1 scope / next steps

The first MVP targets **taking individual photos and pasting them as image attachments**. It does not support silent background iPhone camera access, full desktop app recognition, automatic AI prompt submission, multiple receivers, or text OCR. The most valuable follow-on is a Windows tray app with a short paste queue plus a stable HTTPS PWA origin.
