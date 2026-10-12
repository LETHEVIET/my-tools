# My Tools — minimal Rust Windows background agent

**v0.4.0 and newer:** the background receiver is built into the single **`my-tools.exe`** desktop application. Use `build_hub_windows.bat` to build, `start_hub_windows.bat` to show the dashboard, or `start_windows.bat` to start hidden in the tray. The standalone `rust/` Cargo crate is for development tests only and is not packaged. The historical instructions below may reference the earlier separate agent executable.

**Snap to Paste Image** is the first utility in **My Tools**. The single process also hosts **Prompt Notes**, invoked with **Ctrl+Alt+P**. See [MY_TOOLS.md](MY_TOOLS.md) for the GPUI Kit hub, the notes workflow and the one-EXE architecture.

The default Windows implementation is now a **native Rust executable**. The iPhone page is unchanged, and ships embedded in the binary. No Python, Node.js, Electron, or separate web server is required to run the finished app.

## Build and run

On Windows, install Rust using [rustup](https://rustup.rs/) with the MSVC toolchain, then:

1. Double-click `build_windows.bat` once. It builds **`desktop\target\release\my-tools.exe`**.
2. Double-click `start_windows.bat` to start My Tools hidden in the notification area, or `start_hub_windows.bat` to open the dashboard.
3. On first launch, the My Tools desktop window shows the pairing QR **inside the app**. On later launches, right-click the tray icon and choose **Show pairing QR** to reopen the QR page inside My Tools; no Windows browser tab is needed.
4. Scan the QR with your iPhone Camera app, open the link in Safari, and bookmark it for future snaps.
5. Focus the AI prompt on your Windows PC. **Library** lets you select one or more photos, which paste to the PC in order. On the usual local HTTP link, the round shutter opens the iPhone's native camera picker; confirm **Use Photo** to paste. It does **not** submit your AI prompt.

**Updating an already-running app:** only one process can use port 8765, and Windows won't overwrite a running executable. Double-click `upgrade_windows.bat`, exit the old **Snap to Prompt** or **My Tools** tray process, then continue through the update prompt. This rebuilds the new `my-tools.exe`. Reload Safari after upgrading.

## In-app camera preview (no switching to the iPhone Camera app)

Safari only exposes `navigator.mediaDevices.getUserMedia()` to secure HTTPS origins, not `http://192.168.x.x:8765`. On a trusted HTTPS link the mobile page now automatically starts an **embedded live camera preview** (with iOS camera permission), using the round shutter to snap/send **without leaving Snap to Paste Image**. It offers:

- In-app rear/front camera switching.
- Torch when the iPhone/browser actually supports the web torch constraint.
- Automatic send, then return to the live camera preview for the next snap.
- **Library** button that selects multiple stored images and transfers them in order.
- Client-side conversion to resized JPEG (including HEIC images if supported by the current Safari image decoder).

With plain HTTP, these controls degrade to a native-camera shutter and working Photo Library picker. **An iPhone PWA cannot bypass Safari's HTTPS camera restriction.** See https://developer.mozilla.org/en-US/docs/Web/API/MediaDevices/getUserMedia.

### Try HTTPS with a temporary tunnel

1. Install [cloudflared](https://developers.cloudflare.com/tunnel/get-started/quick-tunnels/) once (for example, `winget install Cloudflare.cloudflared`), then reopen your terminal.
2. Exit the normal My Tools tray agent (only one process can listen on port 8765).
3. Double-click **`start_https_camera.bat`**. It starts a temporary Cloudflare HTTPS tunnel and then the Rust receiver with the corresponding HTTPS pairing URL.
4. Scan the **new** QR code with the iPhone, open it in Safari, allow camera access, and try the embedded shutter. You may use Safari's **Share → Add to Home Screen** so it behaves more like an app.
5. Keep the HTTPS launcher terminal open during this session. When you exit the tray app, its tunnel stops. Reopen using `start_windows.bat` for LAN-only mode.

**Privacy tradeoff:** HTTPS tunnel traffic, including photos, goes through Cloudflare. Cloudflare Quick Tunnels are temporary/test-only and their hostname changes on every launch, so you'll need to rescan QR after restarting the tunnel. For daily always-on use, a stable HTTPS origin (either a trusted local certificate + local network configuration or a secured, named tunnel) is preferable. The app itself stores no photos on your PC.

Use **right-click on the tray icon** to pause automatic pasting, generate a new pairing code, start at Windows login, or exit. When auto-paste is paused, images go to the clipboard only.

If Windows Firewall prompts, allow private Wi-Fi networks only. iPhone and PC should be on the same Wi-Fi, without guest/client isolation. Port 8765 must be free. If upgrading from the previous Python receiver, **exit the Python tray process before launching Rust**. Otherwise both compete for the same port. The agent uses `%LOCALAPPDATA%\MyTools\pairing-token.txt` and automatically imports an existing token from `%LOCALAPPDATA%\SnapToPrompt\pairing-token.txt`.

If you change the PC's local IP or switch from the Python receiver to Rust, rescan the QR once. A new receiver URL has a different Safari local-storage origin.

## Lightweight behavior

- Native Win32 tray icon, message window, image clipboard, SendInput, registry startup toggle.
- Standard-library sockets via `tiny_http`; simple HTTP requests, no always-on WebRTC or WebSocket connection.
- No GUI framework, Python interpreter, WebView, Electron, or Node runtime.
- The iPhone's HTML/JS/CSS/icons are embedded in the release executable. The server stays idle between HTTP requests.
- The main GPUI Kit dashboard renders the pairing QR directly, with no Windows browser needed to display it.
- One-file portable `.exe` in `rust\target\release`, not a PyInstaller unpack-at-start EXE.

Rust compilation is required **only for building**; the resulting executable can be copied to another Windows computer. The Rust toolchain is not needed on the destination PC. For use on startup, keep the executable in its final location before enabling **Start with Windows**.

## Options

Launch the executable with `--port 8765`, `--host 192.168.1.50`, `--public-url https://your-trusted-tunnel.example`, or `--show` to force the pairing page open. The `--public-url` option affects the generated QR address, not the listener; configure the HTTPS reverse proxy/tunnel separately. Photo transfers over local `http://` Wi-Fi are **not encrypted**; only use trusted local networks. HTTPS is required for a fully installed PWA/live camera preview on iPhone; native camera photo selection works on LAN HTTP.

## Verify

```
cargo test --manifest-path rust\Cargo.toml
cargo build --release --manifest-path rust\Cargo.toml
python -m unittest discover -s tests -v
```

The Rust integration test checks the server, QR HTML, auth, and damaged-photo rejection **without simulating Ctrl+V**. Full end-to-end camera, clipboard, and paste validation must be done interactively on your iPhone and Windows PC.

The previous Python version remains available with `start_python_legacy.bat` for comparison/rollback, but is not needed for the Rust receiver.
