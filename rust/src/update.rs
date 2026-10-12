//! GitHub release updates for a per-user My Tools installation.
//!
//! The agent stays tiny: only a periodic check subprocess uses PowerShell and
//! the GitHub CLI. GitHub authentication remains in gh's Windows credential store.
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

const REPOSITORY: &str = "LETHEVIET/my-tools";
const SCRIPT: &str = include_str!("../../scripts/update-my-tools.ps1");

fn app_data() -> Result<PathBuf, String> {
    let root = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable")?;
    Ok(PathBuf::from(root).join("MyTools"))
}

fn installation_dir() -> Result<PathBuf, String> {
    let root = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable")?;
    Ok(PathBuf::from(root).join("Programs").join("MyTools"))
}

pub fn installed() -> bool {
    let Ok(current) = std::env::current_exe() else {
        return false;
    };
    let Ok(destination) = installation_dir() else {
        return false;
    };
    current.parent().is_some_and(|p| {
        p.to_string_lossy()
            .eq_ignore_ascii_case(&destination.to_string_lossy())
    }) && current
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("my-tools.exe"))
}

fn auto_pref_file() -> Result<PathBuf, String> {
    Ok(app_data()?.join("auto-update-disabled"))
}

pub fn auto_enabled() -> bool {
    auto_pref_file().is_ok_and(|path| !path.exists())
}

pub fn set_auto_enabled(enabled: bool) -> Result<(), String> {
    let file = auto_pref_file()?;
    if enabled {
        if file.exists() {
            fs::remove_file(&file).map_err(|e| e.to_string())?;
        }
    } else {
        if let Some(dir) = file.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        fs::write(&file, b"disabled\n").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn updater_file() -> Result<PathBuf, String> {
    let folder = app_data()?;
    fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let path = folder.join("updater.ps1");
    fs::write(&path, SCRIPT).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn check_now(interactive: bool) -> Result<(), String> {
    if !installed() {
        return Err("Automatic updates require an installed My Tools release. Open the downloaded my-tools.exe and choose Install My Tools first.".into());
    }
    let ps = updater_file()?;
    let dir = installation_dir()?;
    let mut command = Command::new("powershell.exe");
    command.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
    ]);
    if !interactive {
        command.args(["-WindowStyle", "Hidden"]);
    }
    command
        .arg("-File")
        .arg(ps)
        .arg("-Repo")
        .arg(REPOSITORY)
        .arg("-CurrentVersion")
        .arg(env!("CARGO_PKG_VERSION"))
        .arg("-InstallDir")
        .arg(dir)
        .arg("-AgentPid")
        .arg(std::process::id().to_string());
    if interactive {
        command.arg("-Interactive");
    }
    command.stdin(Stdio::null());
    if !interactive {
        command.stdout(Stdio::null()).stderr(Stdio::null());
    }
    command
        .spawn()
        .map_err(|e| format!("Cannot start the update checker: {e}"))?;
    Ok(())
}

pub fn spawn_auto_checks() {
    if !installed() {
        return;
    }
    thread::Builder::new()
        .name("github-updates".into())
        .spawn(|| loop {
            if auto_enabled() {
                let _ = check_now(false);
            }
            // Poll very infrequently; gh and PowerShell never stay resident.
            thread::sleep(Duration::from_secs(6 * 60 * 60));
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn embeds_release_verification_and_private_repo() {
        assert_eq!(REPOSITORY, "LETHEVIET/my-tools");
        assert!(SCRIPT.contains("Get-FileHash"));
        assert!(SCRIPT.contains("gh release download"));
    }
}
