# My Tools

One Windows app containing a personal toolbox, written in Rust and GPUI Kit (default theme).

**One executable — `my-tools.exe`.** Double-click to show the desktop hub; close the window to hide to the system tray. Double-click the tray icon to show it again; choose **Exit My Tools** to quit. Image upload, clipboard pasting, and Prompt Notes hotkeys continue to run while the hub is hidden, all within the **same process**.

## Tools

- **Snap to Paste Image:** take or select iPhone photos and paste them directly into the focused Windows AI prompt.
- **Prompt Notes:** press **Ctrl+Alt+P** to compose a longer prompt, then **Ctrl+Enter** to paste back into the prior app. Notes auto-save locally.

## Install

Get the latest **`my-tools.exe`** from your private [GitHub Releases](https://github.com/LETHEVIET/my-tools/releases). Run it directly without an installer script. To enable startup and automatic updates, click **Install My Tools for startup and automatic updates** inside the hub. See [UPDATE_GUIDE.md](UPDATE_GUIDE.md).

## Build from source

On Windows with Rust MSVC, Visual Studio C++ build tools, the Windows SDK, and CMake installed:

```batch
build_hub_windows.bat
start_hub_windows.bat
```

The compiled app is `desktop\target\release\my-tools.exe`. `start_windows.bat` launches it with `--minimized`, directly to the tray. Development startup and build scripts are not included with the downloadable release.

The desktop hub and background tasks share **one binary and one process**. The native agent source in `rust/src/` is included in the desktop build; its smaller standalone test crate is not shipped.

Further details: [MY_TOOLS.md](MY_TOOLS.md), [RUST_SETUP.md](RUST_SETUP.md), [UPDATE_GUIDE.md](UPDATE_GUIDE.md).

**Security:** LAN HTTP photo transfers are not encrypted; live iPhone camera preview requires HTTPS. Prompt Notes drafts are local unencrypted text. Private GitHub updates use `gh auth login` credentials and validate SHA-256 against the release checksum; Windows binaries are not signed yet.
