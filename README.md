# My Tools

A personal collection of lightweight Windows and iPhone utilities, built with Rust and GPUI Kit.

**Currently available**

- **Snap to Paste Image** — take a photo or choose existing photos on iPhone; send the image to Windows and paste into the active AI prompt.
- **Prompt Notes** — press **Ctrl+Alt+P** on Windows to open a scratchpad for longer prompts, then **Ctrl+Enter** to copy and paste back into the previous application. Your draft is saved locally.

The optional GPUI desktop hub uses its default theme. A separate ~sub-megabyte native Rust agent powers the tray, hotkeys, clipboard operations, and iPhone receiver in the background. Closing the hub does not stop these utilities.

## Download and install

Open the **Releases** page in the private repository: https://github.com/LETHEVIET/my-tools/releases

Download **MyTools-windows-x64.zip**, extract the archive, then run the included PowerShell installer in the extracted folder:

    powershell -ExecutionPolicy Bypass -File .\install-my-tools.ps1

This installs My Tools per user, adds a Start Menu shortcut and runs the tray agent at Windows login. No administrator account is needed for installation. For future updates from the private repo, install **GitHub CLI** and authenticate using **gh auth login**. The background agent checks for new releases on startup and approximately every six hours.

## Build from source (Windows)

Install Rust (MSVC target), the Visual Studio C++/Windows SDK toolchain, and CMake.

    build_windows.bat
    build_hub_windows.bat
    start_windows.bat
    start_hub_windows.bat

Developer builds do not auto-update. Only installations made using the release installer do.

## Repository layout

| Directory | Purpose |
| --- | --- |
| `rust/` | Background tray agent; global shortcut, clipboard, notes, image receiver and release updater |
| `desktop/` | GPUI Kit desktop hub, built with the default theme |
| `web/` | iPhone web tools and PWA assets |
| `scripts/` | PowerShell installer and verified GitHub Release updater |
| `.github/workflows/` | Windows CI and tagged-release publishing |
| `tests/` | HTTP, pairing and image processing tests |

See **[MY_TOOLS.md](MY_TOOLS.md)** for features and architecture, **[UPDATE_GUIDE.md](UPDATE_GUIDE.md)** for CI/CD and private-release auto-updates, and **[RUST_SETUP.md](RUST_SETUP.md)** for local photo pairing and HTTPS camera options.

### Security and limitations

LAN HTTP photo transfer is not encrypted; use a trusted private Wi-Fi network, or configure a trusted HTTPS endpoint. The iPhone's in-page live camera requires HTTPS. Prompt Notes drafts are stored as local, unencrypted text. The Windows binaries currently do not have Authenticode signatures. Auto-updates authenticate through your GitHub CLI account, verify the release SHA-256 checksum, and keep the original user data in a separate directory.
