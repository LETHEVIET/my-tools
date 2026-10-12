# My Tools — CI/CD and GitHub auto-updates

The source repository is **private**: https://github.com/LETHEVIET/my-tools. Pushing the main branch runs Windows CI (Rust formatting, both native builds, Rust tests, and HTTP integration tests). Pushing a version tag such as **v0.3.0** publishes a Windows GitHub Release:

- **MyTools-windows-x64.zip** — GPUI desktop hub, Rust background agent, installer script and documentation.
- **SHA256SUMS.txt** — SHA-256 checksum checked by the updater before installing.

The release tag must match the versions in both Cargo.toml manifests. GitHub Actions uses an ephemeral GITHUB_TOKEN to upload releases; no GitHub credentials are checked into source.

## Install once

1. From the private repository's Releases page, download and extract the latest Windows ZIP.
2. In PowerShell, inside the extracted directory, run: powershell -ExecutionPolicy Bypass -File .\install-my-tools.ps1
3. The script installs both EXEs into %LOCALAPPDATA%\Programs\MyTools, creates a Start Menu shortcut, and enables the lightweight agent at Windows login. No administrator privileges or Rust installation are required.
4. Install the GitHub CLI if necessary (winget install GitHub.cli) and sign in with **gh auth login** on this Windows account. The GitHub identity must have access to the **private** repository for the updater to work.

Developer builds launched using start_windows.bat or start_hub_windows.bat are deliberately **not** auto-updated: only installations under %LOCALAPPDATA%\Programs\MyTools are updated. This avoids overwriting development builds.

## How auto-updating works

- The installed tray agent checks GitHub Releases on startup and roughly every six hours, using a short-lived PowerShell and gh subprocess. Neither process stays resident.
- The tray menu includes **Automatic updates** (enabled by default on an installed app) and **Check for updates**.
- When a newer release is available, the updater downloads the release ZIP and checksum from GitHub, validates SHA-256, and confirms the expected executables are present.
- If the GPUI desktop hub is open, installation is **deferred** to avoid closing active work. Close the hub when convenient, and the next check can proceed.
- The updater requests that the tray agent close gracefully so Prompt Notes can save its draft. It then replaces both executables, retains backups during the copy operation, and restarts the agent. Pairing and note files in %LOCALAPPDATA%\MyTools are unaffected.
- Update diagnostics are written to %LOCALAPPDATA%\MyTools\update.log.
- Private-repository updates require an authenticated gh installation. If credentials expire, the background check will log the problem rather than interrupting your work.

The update script itself is embedded in the compiled agent; the newly downloaded ZIP is not allowed to provide a script that runs automatically. ZIP integrity is verified against the checksum published in the matching GitHub release. **Executables are not yet Authenticode-signed.** Before distributing this beyond your own GitHub account, add Windows code signing and signature verification for more robust publisher authentication.

## Publish the next version

When ready, bump the version in both Cargo.toml files, commit and push main, then create and push an annotated version tag:

    git add .
    git commit -m "Release v0.3.1"
    git push origin main
    git tag -a v0.3.1 -m "My Tools v0.3.1"
    git push origin v0.3.1

The **Publish Windows Release** action builds, tests, and uploads the tagged version. My Tools picks it up on a future check. The first published version is **v0.3.0**, so the first update after that must be a higher version such as **v0.3.1**.

## Compatibility and privacy

Windows x64 only. Private GitHub Releases are not anonymously downloadable. The agent reads update assets through your own GitHub CLI account; it does not store a GitHub token in its configuration. Photo pairing and prompt data remain local, except for the optional HTTPS photo tunnel described in RUST_SETUP.md. The update mechanism does not transmit them to GitHub.
