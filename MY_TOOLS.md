# My Tools — desktop hub and first tool

**GitHub CI/CD and the release updater:** see [UPDATE_GUIDE.md](UPDATE_GUIDE.md). The source repository is private and the installable release uses automatic version checks, without leaving a GitHub subprocess running.

**My Tools** is a growing collection of utilities for Windows and iPhone. This is the **application name**, not the name of an individual utility. The first tool is **Snap to Paste Image**: snap or select an iPhone image and paste it automatically into the focused Windows prompt. The second is **Prompt Notes**, a quick Windows scratchpad for composing longer prompts before pasting them into your AI assistant.

## Application naming

| Component | Executable | Display name |
| --- | --- | --- |
| Desktop hub (GPUI Kit) | `desktop\target\release\my-tools.exe` | **My Tools** |
| Small tray/HTTP/shortcut process | `rust\target\release\my-tools-agent.exe` | **My Tools — Background Agent** |
| iPhone photo utility | Inside the hub | **Snap to Paste Image** |
| Windows prompt utility | Inside the hub | **Prompt Notes** |

The app's folder on disk can remain `C:\code\snap-to-prompt`: renaming the repository folder is optional and is **not** necessary for the user-facing rebrand. The old `snap-to-prompt.exe` build is obsolete and should be exited before running the new agent.

To avoid losing paired iPhones, the agent copies the existing pairing secret from `%LOCALAPPDATA%\SnapToPrompt\pairing-token.txt` to `%LOCALAPPDATA%\MyTools\pairing-token.txt` on first launch; the old file stays intact for rollback. It also updates the previous `SnapToPrompt` Windows startup registry entry to `MyTools` when present. Existing Safari pairing credentials and the saved Prompt Notes file stay unchanged.

## Prompt Notes — shortcut scratchpad

**Ctrl+Alt+P** opens a small native Windows text window from anywhere, even when the GPUI hub is closed. Keep the tiny Rust receiver running in the system tray. The text window and its hotkey are part of the same receiver process, so no extra resident app or runtime is needed.

1. On Windows, focus the ChatGPT, Claude, or other AI **prompt input**.
2. Press **Ctrl+Alt+P**. Type or edit your longer prompt in **Prompt Notes**.
3. Press **Ctrl+Enter** or click **Paste to AI**. The note is copied to the Windows clipboard, the previous application is reactivated, and Ctrl+V is sent. Your prompt is **not submitted automatically**.
4. **Ctrl+S** saves without leaving, and **Esc** hides the scratchpad. **Copy only** copies the note without attempting to paste. The **Prompt Notes** tray-menu item and **Open Prompt Notes** button in the GPUI hub can open it, too; use **Copy only** if it was opened from the hub or tray instead of from your intended AI app.

Drafts auto-save while typing and are restored after restarting Windows. They live in `%LOCALAPPDATA%\MyTools\prompt-notes.txt` **as local, unencrypted plain text**. Avoid storing passwords or sensitive information. The last draft remains available after pasting so you can reuse and improve it.

If Ctrl+Alt+P conflicts with another app, the receiver displays a warning and the **Prompt Notes** tray menu still works. Windows may prevent simulated Ctrl+V in elevated apps or if the previous window disappeared; in that case, the text remains on the clipboard for manual pasting. If you close the note without pasting, your draft remains saved.

The hub uses GPUI Kit's built-in default theme. The always-on Prompt Notes popup intentionally uses Windows' native dialog controls so it has no GPUI runtime cost when the hub is closed.

## App layout

| Component | Location | Responsibility |
| --- | --- | --- |
| My Tools desktop hub | `desktop/` | GPUI Kit native desktop window; default built-in theme and components; start receiver and open pairing QR |
| Background receiver | `rust/` | Tiny standalone Win32 tray and HTTP photo receiver; does not load GPUI Kit |
| Prompt Notes | `rust/src/notes.rs` | Global hotkey, small native scratchpad, autosave, text clipboard and paste-back |
| iPhone tools index | `web/index.html` | Launch available mobile tools |
| Snap to Paste Image | `web/tools/snap-to-paste-image/` | Camera and library picker, image upload |
| Backend tool catalog | `rust/src/tools.rs` | Stable tool names and API routes |

## Windows run steps

1. Build the small tray agent with `build_windows.bat` and start it with `start_windows.bat`. To replace an already-running old EXE, exit it from the tray first (or use `upgrade_windows.bat`).
2. Run `build_hub_windows.bat` and `start_hub_windows.bat` to open the native **My Tools** desktop hub. The **Prompt Notes** shortcut works without opening this hub.
3. GPUI Kit requires Rust MSVC, Windows SDK, Visual Studio C++ Build Tools, and CMake. Install CMake with `winget install Kitware.CMake`. If working inside this repo's `.venv`, `python -m pip install cmake` is also an option; the build script detects `.venv\Scripts\cmake.exe`.
4. In **My Tools**, choose **Pair iPhone** to open the QR code. On your phone, the `My Tools` page lists **Snap to Paste Image** as its first tool.
5. Leave the small receiver running in the system tray even after closing the hub window. The GPUI hub does not need to remain open for photo uploads.

GPUI Kit controls use its built-in default theme, including system light/dark preferences. This project deliberately does not ship a custom theme or include GPUI Shell's optional JavaScript plugin runtime.

The current desktop hub uses port `8765` for its local receiver. The mobile image tool has its own namespaced endpoints (`/api/tools/snap-to-paste-image/status` and `/api/tools/snap-to-paste-image/photo`) while the old `/api/status` and `/api/photo` endpoints remain as compatibility aliases. Prompt Notes is Windows-only and does not expose draft text over HTTP.

## Adding another tool

1. Give it a unique slug (for example `text-share`) and add a separate folder under `web/tools/` for the mobile experience, if needed.
2. Register its routes and name in `rust/src/tools.rs`, then implement server handlers in `rust/src/receiver.rs` or a separate Rust tool module.
3. Add it to the mobile index and GPUI hub. Shared receiver, authentication, settings, and native integrations should stay in the core rather than be copied into each tool.
4. Add route/authentication tests before shipping it.

**Security:** The receiver is for trusted LAN use; HTTP photo transfers are not encrypted. The optional HTTPS tunnel documented in `RUST_SETUP.md` routes photos through its provider. On iPhone, embedded camera mode still requires an HTTPS origin; the native camera picker works over HTTP.

## Verification

```
cargo test --manifest-path rust\Cargo.toml
cargo check --locked --manifest-path desktop\Cargo.toml
python -m unittest discover -s tests -v
```

The desktop UI requires testing visually on Windows and the actual Ctrl+V flow still needs an interactive focused-app test after changes. The GPUI executable is optional; only the small tray receiver stays running for background work.
