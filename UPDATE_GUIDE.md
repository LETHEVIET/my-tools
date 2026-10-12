# My Tools — single EXE and GitHub updates

**Release download:** `my-tools.exe` from the private [GitHub Releases page](https://github.com/LETHEVIET/my-tools/releases). The companion `SHA256SUMS.txt` is a verification file, **not another app**. There is no installer `.ps1` in the release.

## Open, hide, and quit

1. Double-click `my-tools.exe` to open the **My Tools** desktop hub. It includes the photo-transfer receiver and Prompt Notes in the **same Windows process**.
2. Click the **X** to hide My Tools to the Windows notification area (system tray). iPhone photo pasting and **Ctrl+Alt+P** Prompt Notes remain available.
3. Double-click the tray icon or right-click and select **Open My Tools** to show the hub again. Running `my-tools.exe` a second time also brings back the existing hub instead of starting another copy.
4. Right-click the tray icon and choose **Exit My Tools** to quit completely.

## Optional one-click installation

Run the downloaded EXE, then choose **Install My Tools for startup and automatic updates** in the hub. The app copies **one** file to `%LOCALAPPDATA%\Programs\MyTools\my-tools.exe` and registers itself to start **hidden to tray** after Windows login. Restart using this installed copy for future automatic updates. No administrator permissions or external installer script are required.

**Upgrading from v0.3.0 (the old two-EXE release):** exit both the old My Tools desktop window and its old tray agent first, then download `my-tools.exe` v0.4.0 and click **Install My Tools** in the new dashboard. This is a **one-time manual upgrade** because the v0.3.0 updater expects the old ZIP package format. Subsequent single-EXE releases can update automatically. Existing pairing and Prompt Notes files are preserved; the installer removes the obsolete `my-tools-agent.exe` from the installation folder when Windows permits it.

For the private repository, install and authenticate [GitHub CLI](https://cli.github.com/) on that Windows user account using `gh auth login`. The CLI is called only during update checks, and its credentials remain in the Windows credential store.

## Automatic updates

The installed program checks for new GitHub Releases on startup and approximately every six hours. **Check for updates** and **Automatic updates** are available from the tray menu. The update helper is stored inside the EXE and extracted temporarily when checking; it is not distributed as a separate app. When a newer release appears:

- Download the new `my-tools.exe` plus `SHA256SUMS.txt` using the authenticated GitHub CLI.
- Verify the downloaded executable's SHA-256 value against the release metadata.
- **Defer the upgrade while the desktop hub is visible**, avoiding interruptions. Once it's hidden, a future check can install the release.
- Save the Prompt Notes draft, gracefully shut down the one process, back up the old EXE, replace it, and restart with `--minimized`. Pairing settings and prompt notes in `%LOCALAPPDATA%\MyTools` are untouched.

Diagnostic log: `%LOCALAPPDATA%\MyTools\update.log`.

The GitHub account must retain access to the private repository for updates. The binaries are **not Authenticode-signed yet**, so distribute only to trusted users until publisher signatures are available.

## CI/CD

Pushing to `main` runs the Windows build and tests. Pushing a new `vMAJOR.MINOR.PATCH` tag triggers the Windows Release workflow. Both `desktop/Cargo.toml` and `rust/Cargo.toml` must match the tag version. The workflow publishes **one EXE**, plus its SHA-256 checksum.

For example, after editing the code and bumping both manifest versions to `0.4.1`:

```powershell
git add -A
git commit -m "Release v0.4.1"
git push origin main
git tag -a v0.4.1 -m "My Tools v0.4.1"
git push origin v0.4.1
```

Developer builds (`build_windows.bat`, `start_windows.bat`, `start_hub_windows.bat`) are not automatically replaced during updates. All feature behavior is inside the `desktop/target/release/my-tools.exe` binary, even though the repository keeps a small `rust/` crate for testing native helper logic.
