//! Optional one-click, per-user install. Only one EXE is copied.
use std::fs;
use std::path::PathBuf;

use windows_sys::Win32::System::Registry::{
    RegDeleteKeyValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ,
};

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

pub fn install_dir() -> Result<PathBuf, String> {
    let root = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable")?;
    Ok(PathBuf::from(root).join("Programs").join("MyTools"))
}

pub fn is_installed() -> bool {
    let Ok(current) = std::env::current_exe() else {
        return false;
    };
    let Ok(dir) = install_dir() else { return false };
    current.parent().is_some_and(|p| {
        p.to_string_lossy()
            .eq_ignore_ascii_case(&dir.to_string_lossy())
    })
}

pub fn install() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let destination = install_dir()?;
    fs::create_dir_all(&destination).map_err(|e| format!("Cannot create install folder: {e}"))?;
    let target = destination.join("my-tools.exe");
    if !is_installed() {
        fs::copy(&exe, &target).map_err(|e| format!("Cannot install My Tools: {e}"))?;
    }
    // Previous releases shipped a second agent binary. It is obsolete once
    // the startup command below points to the unified executable.
    let obsolete_agent = destination.join("my-tools-agent.exe");
    if obsolete_agent.exists() {
        let _ = fs::remove_file(&obsolete_agent); // May be locked until the old agent exits.
    }
    let startup_cmd = format!("\"{}\" --minimized", target.display());
    let run_key = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    let data = wide(&startup_cmd);
    let result = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            run_key.as_ptr(),
            wide("MyTools").as_ptr(),
            REG_SZ,
            data.as_ptr().cast(),
            (data.len() * 2) as u32,
        )
    };
    if result != 0 {
        return Err(format!("Cannot configure Windows startup: error {result}"));
    }
    unsafe {
        RegDeleteKeyValueW(
            HKEY_CURRENT_USER,
            run_key.as_ptr(),
            wide("SnapToPrompt").as_ptr(),
        );
    }
    Ok(format!(
        "My Tools installed at {}. It will start in the tray at Windows login. Run the installed my-tools.exe to open the dashboard. The current window can be closed normally.",
        target.display()
    ))
}
