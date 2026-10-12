#![cfg_attr(windows, windows_subsystem = "windows")]

use gpui_kit::*;
use std::net::{SocketAddr, TcpStream};
use std::process::Command;
use std::time::Duration;

// One executable and one process. The Win32 tray runs on a background thread;
// GPUI remains on the primary UI thread.
#[cfg(windows)]
#[path = "../../rust/src/agent.rs"]
mod agent;
#[cfg(windows)]
mod install;
#[cfg(windows)]
#[path = "../../rust/src/notes.rs"]
mod notes;
#[cfg(windows)]
#[path = "../../rust/src/quick_notes_store.rs"]
mod quick_notes_store;
#[cfg(windows)]
#[path = "../../rust/src/receiver.rs"]
mod receiver;
#[cfg(windows)]
#[path = "../../rust/src/tools.rs"]
mod tools;
#[cfg(windows)]
#[path = "../../rust/src/update.rs"]
mod update;
#[cfg(windows)]
#[path = "../../rust/src/win.rs"]
mod win;

const RECEIVER_PORT: u16 = 8765;

fn receiver_online() -> bool {
    TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], RECEIVER_PORT)),
        Duration::from_millis(180),
    )
    .is_ok()
}

fn pairing_url() -> String {
    format!("http://127.0.0.1:{RECEIVER_PORT}/pair")
}

#[cfg(windows)]
fn open_notes(quick: bool) -> Result<&'static str, String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, PostMessageW, WM_APP};
    let classname: Vec<u16> = "MyToolsAgentWin32".encode_utf16().chain(Some(0)).collect();
    // The receiver starts almost instantly, but allow a brief moment for its
    // native message window to be registered before opening the scratchpad.
    for _ in 0..24 {
        unsafe {
            let hwnd = FindWindowW(classname.as_ptr(), std::ptr::null());
            if !hwnd.is_null() {
                if PostMessageW(hwnd, WM_APP + if quick { 44 } else { 42 }, 0, 0) == 0 {
                    return Err("Could not open Prompt Notes. Try Ctrl+Alt+P.".into());
                }
                return Ok(if quick {
                    "Quick Note opened. Ctrl+S saves a new local note, without pasting."
                } else {
                    "Prompt Notes opened. Use Ctrl+Alt+P while an AI prompt is focused for paste-back."
                });
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("Prompt Notes is starting. Exit any older Snap to Prompt tray process and relaunch My Tools.".into())
}

#[cfg(not(windows))]
fn open_notes(_quick: bool) -> Result<&'static str, String> {
    Err("Prompt Notes currently requires Windows.".into())
}

#[cfg(windows)]
fn open_browser(url: &str) -> Result<(), String> {
    // Explorer uses the user's registered default browser and is not kept running by this app.
    Command::new("explorer.exe")
        .arg(url)
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(not(windows))]
fn open_browser(_url: &str) -> Result<(), String> {
    Err("Desktop launcher currently targets Windows".into())
}

mod hub;
use hub::Hub;

fn main() {
    #[cfg(windows)]
    {
        if std::env::args().any(|arg| arg == "--install") {
            match install::install() {
                Ok(message) => win::info_dialog(&message),
                Err(error) => win::error_dialog(&error),
            }
            return;
        }
        // Test-only mode for headless Windows CI runners. Still the same EXE,
        // but skips GPUI initialization when no interactive desktop exists.
        if std::env::args().any(|arg| arg == "--headless-test") {
            if let Err(error) = agent::run() {
                win::error_dialog(&error);
            }
            return;
        }
        // A second launch simply reveals the already running desktop window.
        if win::another_instance_running() {
            win::show_hub();
            return;
        }
        std::thread::Builder::new()
            .name("my-tools-tray".into())
            .spawn(|| {
                if let Err(error) = agent::run() {
                    win::error_dialog(&error);
                }
            })
            .expect("Cannot start tray thread");
    }
    let initially_hidden = std::env::args().any(|arg| arg == "--minimized");
    application().with_assets(assets::Assets).run(move |cx| {
        init(cx);
        let options = WindowOptions {
            titlebar: Some(TitlebarOptions {
                title: Some(win::hub_title().into()),
                ..Default::default()
            }),
            show: !initially_hidden,
            ..WindowOptions::default()
        };
        open_window(options, cx, |window, cx| {
            #[cfg(windows)]
            window.on_window_should_close(cx, |_, cx| {
                if win::exiting() {
                    cx.quit();
                    true
                } else {
                    win::hide_hub();
                    false
                }
            });
            cx.new(|_| Hub::new())
        })
        .expect("Unable to open My Tools");
    });
}
