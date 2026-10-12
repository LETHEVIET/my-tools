#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(not(windows))]
fn main() {
    eprintln!("My Tools background agent runs on Windows only.");
}

#[cfg(windows)]
mod agent;
#[cfg(windows)]
mod notes;
#[cfg(windows)]
mod receiver;
#[cfg(windows)]
mod tools;
#[cfg(windows)]
mod update;
#[cfg(windows)]
mod win;

#[cfg(windows)]
fn main() {
    if let Err(error) = agent::run() {
        win::error_dialog(&format!("My Tools could not start:\n{error}"));
    }
}
