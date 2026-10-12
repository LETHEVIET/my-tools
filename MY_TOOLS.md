# My Tools — desktop hub and first tool

**Since v0.4.0 My Tools distributes exactly one executable:** `my-tools.exe`. It opens the GPUI Kit dashboard when double-clicked. Closing the window hides to the system tray; double-click the tray icon to reopen. The HTTP image receiver and global Prompt Notes hotkey operate on background threads in the same process. Choose **Exit My Tools** in the tray menu to stop it. `my-tools.exe --minimized` launches directly into the tray. The native Rust modules remain separate in source for maintainability, but are linked into the **same executable**.

The dashboard now uses GPUI Kit's default-theme **Sidebar** shell, with pages for
Overview, each tool, **Saved Notes** and Settings. Saved Notes now uses
**SQLite with FTS5 keyword search**, editable titles/bodies/tags, and pin-to-top
controls. Saved notes live in `%LOCALAPPDATA%\MyTools\my-tools.db`; existing
legacy `.txt` notes are imported once and never deleted. Use **Data folder**
to open local storage. See [NOTES_DATABASE.md](NOTES_DATABASE.md).
The main window defaults to a compact 860 × 590 size, while Prompt Notes and
Quick Note each use a 560 × 405 GPUI Kit-themed editor window.
The SVG logo source is `desktop/assets/my-tools.svg`; generated
`desktop/assets/my-tools.ico` is included as an EXE resource and in the tray.
The same artwork supplies the iPhone home-screen icons.

**Quick Launcher and Quick Tasks (v0.8.0):** Press **Ctrl+Shift+Space** to open
a floating GPUI Kit palette for tools, saved notes, and tasks. The new
**Quick Tasks** sidebar page captures tasks with optional due times and
shows Today/Upcoming/Completed views. The tray checks for due tasks every
30 seconds while My Tools is running. See
[QUICK_LAUNCHER_TASKS.md](QUICK_LAUNCHER_TASKS.md).

The download is the executable alone; no second EXE, ZIP unpacking, or installer script is required. Optionally click **Install My Tools** inside the app to copy itself to `%LOCALAPPDATA%\Programs\MyTools` and enable tray startup and public GitHub HTTPS updates. See [UPDATE_GUIDE.md](UPDATE_GUIDE.md).

**GitHub CI/CD and the release updater:** see [UPDATE_GUIDE.md](UPDATE_GUIDE.md). The source repository and release downloads are public. The installed app checks for updates over HTTPS without GitHub CLI or authentication.

**My Tools** is a growing collection of utilities for Windows and iPhone. This is the **application name**, not the name of an individual utility. The first tool is **Snap to Paste Image**: snap or select an iPhone image and paste it automatically into the focused Windows prompt. The second is **Prompt Notes**, a quick Windows scratchpad for composing longer prompts before pasting them into your AI assistant.

## Application naming

| Component | Executable | Display name |
| --- | --- | --- |
| GPUI Kit desktop and Win32 tray | `desktop\target\release\my-tools.exe` | **My Tools** (one process) |
| iPhone photo utility | Inside the hub | **Snap to Paste Image** |
| Windows prompt utility | Inside the hub | **Prompt Notes** |

The app's folder on disk can remain `C:\code\snap-to-prompt`: renaming the repository folder is optional and is **not** necessary for the user-facing rebrand. The old `snap-to-prompt.exe` build is obsolete and should be exited before running the new agent.

To avoid losing paired iPhones, the agent copies the existing pairing secret from `%LOCALAPPDATA%\SnapToPrompt\pairing-token.txt` to `%LOCALAPPDATA%\MyTools\pairing-token.txt` on first launch; the old file stays intact for rollback. It also updates the previous `SnapToPrompt` Windows startup registry entry to `MyTools` when present. Existing Safari pairing credentials and the saved Prompt Notes file stay unchanged.

## Prompt Notes — shortcut scratchpad

**Ctrl+Alt+P** opens a GPUI Kit-themed text editor from anywhere, even when the main window is hidden. The shortcut runs in the same My Tools process, with no additional app required.

1. On Windows, focus the ChatGPT, Claude, or other AI **prompt input**.
2. Press **Ctrl+Alt+P**. Type or edit your longer prompt in **Prompt Notes**.
3. Press **Ctrl+Enter** or click **Paste to AI**. The note is copied to the Windows clipboard, the previous application is reactivated, and Ctrl+V is sent. Your prompt is **not submitted automatically**.
4. **Ctrl+S** saves the text to **Saved Notes** and **closes the note window**, while retaining the reusable Prompt Notes draft; **Esc** closes it without deleting the automatically saved draft. **Copy only** copies the note without pasting. The tray-menu item and desktop hub can open the editor too; use **Copy only** when no target AI prompt was focused first.

Drafts auto-save while typing and are restored after restarting Windows. They live in `%LOCALAPPDATA%\MyTools\prompt-notes.txt` **as local, unencrypted plain text**. Avoid storing passwords or sensitive information. The last draft remains available after pasting so you can reuse and improve it.

If Ctrl+Alt+P conflicts with another app, the receiver displays a warning and the **Prompt Notes** tray menu still works. Windows may prevent simulated Ctrl+V in elevated apps or if the previous window disappeared; in that case, the text remains on the clipboard for manual pasting. If you close the note without pasting, your draft remains saved.

The hub and both note editors use GPUI Kit's built-in default theme, Textarea and Button components. There is still one executable and one process. GPUI stays loaded when the dashboard is hidden, so the shortcuts can open these themed windows instantly.

## Quick Note — save without pasting

Press **Ctrl+Alt+N** anywhere on Windows, type your note, then press **Ctrl+S** (or click **Save Note**) to save and **close** the popup. Unlike Prompt Notes, **Quick Note never writes to your clipboard or sends Ctrl+V**. Each saved note becomes a record in local SQLite. Use **Saved Notes** in the main window to search, edit, tag or pin notes; **Data folder** opens the local database directory.

Close the note window with **Esc** to keep an unsaved Quick Note as a separate, auto-saved draft. You can return later without losing it. Saved Quick Notes and the Prompt Notes draft remain separate. Everything is stored locally, unencrypted, so don't put passwords or secrets in them. The themed editor works even when the main window is hidden in the system tray.

## App layout

| Component | Location | Responsibility |
| --- | --- | --- |
| My Tools desktop hub | `desktop/` | GPUI Kit native desktop window; default built-in theme and components; start receiver and open pairing QR |
| Background services (linked into `my-tools.exe`) | `rust/` | Win32 tray, global hotkey, and HTTP receiver on background threads in the same process |
| Prompt Notes | `desktop/src/note_window.rs`, `rust/src/notes.rs` | GPUI Kit editor, global hotkey, autosave, text clipboard and paste-back |
| Tray-to-GPUI events | `desktop/src/ui_bridge.rs` | Routes pairing and global note shortcuts onto the GPUI foreground thread |
| iPhone tools index | `web/index.html` | Launch available mobile tools |
| Snap to Paste Image | `web/tools/snap-to-paste-image/` | Camera and library picker, image upload |
| Backend tool catalog | `rust/src/tools.rs` | Stable tool names and API routes |

## Windows run steps

1. Build the single app with `build_hub_windows.bat` (or `build_windows.bat`). Exit any old version from the tray before replacing its running executable.
2. Run `start_hub_windows.bat` to open **My Tools**, or `start_windows.bat` to start hidden in the tray. The **Prompt Notes** shortcut works even when the dashboard is hidden.
3. GPUI Kit requires Rust MSVC, Windows SDK, Visual Studio C++ Build Tools, and CMake. Install CMake with `winget install Kitware.CMake`. If working inside this repo's `.venv`, `python -m pip install cmake` is also an option; the build script detects `.venv\Scripts\cmake.exe`.
4. In **My Tools**, choose **Pair iPhone** to display the QR directly inside the desktop Photo tool page—no Windows browser tab is opened. Scan with your iPhone and open the iPhone web app as normal.
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

The desktop UI and the actual Ctrl+V flow still benefit from an interactive focused-app test after changes. My Tools always uses one process: the GPUI window can hide to the tray, where the image receiver and note shortcuts continue to run.
