# My Tools

One Windows app containing a personal toolbox, written in Rust and GPUI Kit (default theme).

The desktop hub uses a native **GPUI Kit sidebar shell** with Overview, individual
tool pages, **Saved Notes**, and Settings. Saved Notes lets you browse and read
Quick Notes without changing the clipboard. The app's icon is embedded in
`my-tools.exe` and shown in the tray, and the iPhone launcher uses matching
icons. The editable vector source is `desktop/assets/my-tools.svg`.

**One executable — `my-tools.exe`.** Double-click to show the desktop hub; close the window to hide to the system tray. Double-click the tray icon to show it again; choose **Exit My Tools** to quit. Image upload, clipboard pasting, and Prompt Notes hotkeys continue to run while the hub is hidden, all within the **same process**.

## Tools

- **Snap to Paste Image:** take or select iPhone photos and paste them directly into the focused Windows AI prompt.
- **Prompt Notes:** press **Ctrl+Alt+P** to compose a longer prompt, then **Ctrl+Enter** to paste back into the prior app. Notes auto-save locally.
- **Quick Note:** press **Ctrl+Alt+N** to capture an idea. Use **Save Note** or **Ctrl+S** to store it in your local notes collection without copying or pasting anything. Double-click a saved note in the popup to reopen it.

You can also open **Saved Notes** from the left sidebar and select any note to
read it in full, or choose **Open folder** to work with the underlying plain
text files in File Explorer.

## Install

Get the latest **`my-tools.exe`** from the public [GitHub Releases](https://github.com/LETHEVIET/my-tools/releases). Run it directly without an installer script. To enable startup and automatic updates, click **Install My Tools for startup and automatic updates** inside the hub. See [UPDATE_GUIDE.md](UPDATE_GUIDE.md). Updates download over HTTPS without GitHub CLI or authentication.

## Build from source

On Windows with Rust MSVC, Visual Studio C++ build tools, the Windows SDK, and CMake installed:

```batch
build_hub_windows.bat
start_hub_windows.bat
```

The compiled app is `desktop\target\release\my-tools.exe`. `start_windows.bat` launches it with `--minimized`, directly to the tray. Development startup and build scripts are not included with the downloadable release.

The desktop hub and background tasks share **one binary and one process**. The native agent source in `rust/src/` is included in the desktop build; its smaller standalone test crate is not shipped.

Further details: [MY_TOOLS.md](MY_TOOLS.md), [RUST_SETUP.md](RUST_SETUP.md), [UPDATE_GUIDE.md](UPDATE_GUIDE.md).

**Security:** LAN HTTP photo transfers are not encrypted; live iPhone camera preview requires HTTPS. Prompt Notes drafts and Quick Notes are local unencrypted text. Public GitHub updates download over HTTPS and validate SHA-256 against the release checksum; Windows binaries are not signed yet.
