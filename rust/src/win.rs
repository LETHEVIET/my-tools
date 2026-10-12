//! Minimal Win32 tray, startup preference, CF_DIB clipboard, and SendInput.
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use image::DynamicImage;
use windows_sys::Win32::Foundation::{GlobalFree, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows_sys::Win32::System::Ole::CF_DIB;
use windows_sys::Win32::System::Registry::{
    RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL,
};
use windows_sys::Win32::UI::Shell::{
    ShellExecuteW, Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
    NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetForegroundWindow, GetMessageW, LoadIconW, MessageBoxW,
    PostQuitMessage, RegisterClassW, SetForegroundWindow, TrackPopupMenu, TranslateMessage,
    IDI_APPLICATION, MB_ICONERROR, MB_OK, MF_CHECKED, MF_SEPARATOR, MF_STRING, MSG, SW_SHOWNORMAL,
    TPM_RIGHTBUTTON, WM_COMMAND, WM_DESTROY, WM_LBUTTONDBLCLK, WM_RBUTTONUP, WM_USER, WNDCLASSW,
};

use crate::receiver::State;

const TRAY_CALLBACK: u32 = WM_USER + 29;
const SHOW: usize = 1001;
const TOGGLE: usize = 1002;
const ROTATE: usize = 1003;
const AUTOSTART: usize = 1004;
const EXIT: usize = 1005;
const RUN_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_NAME: &str = "SnapToPrompt";

static STATE: OnceLock<Arc<State>> = OnceLock::new();
static PORT: OnceLock<u16> = OnceLock::new();
static WINDOW: AtomicIsize = AtomicIsize::new(0);

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

pub fn error_dialog(message: &str) {
    unsafe {
        MessageBoxW(
            null_mut(),
            wide(message).as_ptr(),
            wide("Snap to Prompt").as_ptr(),
            MB_ICONERROR | MB_OK,
        );
    }
}

fn startup_cmd() -> String {
    format!(
        "\"{}\"",
        std::env::current_exe().unwrap_or_default().display()
    )
}

fn start_on_login() -> bool {
    let mut buffer = [0u16; 2048];
    let mut size = (buffer.len() * 2) as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            wide(RUN_PATH).as_ptr(),
            wide(RUN_NAME).as_ptr(),
            RRF_RT_REG_SZ,
            null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut size,
        )
    };
    result == 0
        && String::from_utf16_lossy(
            &buffer[..buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len())],
        ) == startup_cmd()
}

fn toggle_start_on_login() -> Result<(), String> {
    let result = if start_on_login() {
        unsafe {
            RegDeleteKeyValueW(
                HKEY_CURRENT_USER,
                wide(RUN_PATH).as_ptr(),
                wide(RUN_NAME).as_ptr(),
            )
        }
    } else {
        let value = wide(&startup_cmd());
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                wide(RUN_PATH).as_ptr(),
                wide(RUN_NAME).as_ptr(),
                REG_SZ,
                value.as_ptr().cast(),
                (value.len() * 2) as u32,
            )
        }
    };
    if result == 0 {
        Ok(())
    } else {
        Err(format!("Windows startup preference failed ({result})"))
    }
}

fn show_pairing() {
    let port = *PORT.get().expect("tray port initialized");
    let link = wide(&format!("http://127.0.0.1:{port}/pair"));
    unsafe {
        ShellExecuteW(
            null_mut(),
            wide("open").as_ptr(),
            link.as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        );
    }
}

fn notify_icon(hwnd: HWND, add: bool) -> bool {
    let mut icon: NOTIFYICONDATAW = unsafe { zeroed() };
    icon.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    icon.hWnd = hwnd;
    icon.uID = 1;
    icon.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    icon.uCallbackMessage = TRAY_CALLBACK;
    icon.hIcon = unsafe { LoadIconW(null_mut(), IDI_APPLICATION) };
    let tip = wide("Snap to Prompt - iPhone photo receiver");
    icon.szTip[..tip.len()].copy_from_slice(&tip);
    unsafe { Shell_NotifyIconW(if add { NIM_ADD } else { NIM_DELETE }, &icon) != 0 }
}

fn menu(hwnd: HWND) {
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        AppendMenuW(menu, MF_STRING, SHOW, wide("Show pairing QR").as_ptr());
        if let Some(state) = STATE.get() {
            AppendMenuW(
                menu,
                MF_STRING
                    | if state.auto_paste.load(Ordering::Relaxed) {
                        MF_CHECKED
                    } else {
                        0
                    },
                TOGGLE,
                wide("Auto-paste into focused prompt").as_ptr(),
            );
        }
        AppendMenuW(menu, MF_STRING, ROTATE, wide("New pairing code").as_ptr());
        AppendMenuW(
            menu,
            MF_STRING | if start_on_login() { MF_CHECKED } else { 0 },
            AUTOSTART,
            wide("Start with Windows").as_ptr(),
        );
        AppendMenuW(menu, MF_SEPARATOR, 0, null());
        AppendMenuW(menu, MF_STRING, EXIT, wide("Exit Snap to Prompt").as_ptr());
        let mut point: POINT = zeroed();
        GetCursorPos(&mut point);
        SetForegroundWindow(hwnd);
        TrackPopupMenu(menu, TPM_RIGHTBUTTON, point.x, point.y, 0, hwnd, null());
        DestroyMenu(menu);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        TRAY_CALLBACK => {
            match lparam as u32 {
                WM_RBUTTONUP => menu(hwnd),
                WM_LBUTTONDBLCLK => show_pairing(),
                _ => {}
            }
            0
        }
        WM_COMMAND => {
            match wparam & 0xFFFF {
                SHOW => show_pairing(),
                TOGGLE => {
                    if let Some(s) = STATE.get() {
                        s.auto_paste.fetch_xor(true, Ordering::Relaxed);
                    }
                }
                ROTATE => {
                    if let Some(s) = STATE.get() {
                        if let Err(e) = s.rotate() {
                            error_dialog(&e);
                        } else {
                            show_pairing();
                        }
                    }
                }
                AUTOSTART => {
                    if let Err(e) = toggle_start_on_login() {
                        error_dialog(&e);
                    }
                }
                EXIT => {
                    DestroyWindow(hwnd);
                }
                _ => {}
            }
            0
        }
        WM_DESTROY => {
            notify_icon(hwnd, false);
            WINDOW.store(0, Ordering::Relaxed);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn run_tray(state: Arc<State>, port: u16, show: bool) -> Result<(), String> {
    STATE
        .set(state)
        .map_err(|_| "Tray already running".to_string())?;
    PORT.set(port)
        .map_err(|_| "Tray already configured".to_string())?;
    let class_name = wide("SnapToPromptTrayWin32");
    let mut class: WNDCLASSW = unsafe { zeroed() };
    class.lpfnWndProc = Some(wndproc);
    class.lpszClassName = class_name.as_ptr();
    unsafe {
        if RegisterClassW(&class) == 0 {
            return Err("Cannot register tray window".into());
        }
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            wide("Snap to Prompt").as_ptr(),
            0,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            null_mut(),
            null(),
        );
        if hwnd.is_null() {
            return Err("Cannot create tray message window".into());
        }
        WINDOW.store(hwnd as isize, Ordering::Relaxed);
        if !notify_icon(hwnd, true) {
            return Err("Cannot add Windows tray icon".into());
        }
        if show {
            show_pairing();
        }
        let mut message: MSG = zeroed();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
    Ok(())
}

fn bitmap_dib(image: &DynamicImage) -> Vec<u8> {
    let rgb = image.to_rgb8();
    let (width, height) = rgb.dimensions();
    let stride = ((width as usize * 3 + 3) / 4) * 4;
    let mut dib = Vec::with_capacity(40 + stride * height as usize);
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(width as i32).to_le_bytes());
    dib.extend_from_slice(&(height as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&24u16.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&((stride * height as usize) as u32).to_le_bytes());
    dib.extend_from_slice(&[0u8; 16]);
    let pixels = rgb.as_raw();
    for y in (0..height as usize).rev() {
        for px in pixels[y * width as usize * 3..(y + 1) * width as usize * 3].chunks_exact(3) {
            dib.extend_from_slice(&[px[2], px[1], px[0]]);
        }
        dib.resize(dib.len() + stride - width as usize * 3, 0);
    }
    dib
}

pub fn copy_and_paste(image: &DynamicImage, auto: bool) -> Result<&'static str, String> {
    let dib = bitmap_dib(image);
    unsafe {
        let handle = GlobalAlloc(GMEM_MOVEABLE, dib.len());
        if handle.is_null() {
            return Err("Cannot allocate clipboard memory".into());
        }
        let ptr = GlobalLock(handle);
        if ptr.is_null() {
            GlobalFree(handle);
            return Err("Cannot lock clipboard memory".into());
        }
        std::ptr::copy_nonoverlapping(dib.as_ptr(), ptr.cast::<u8>(), dib.len());
        GlobalUnlock(handle);
        let mut opened = false;
        for _ in 0..15 {
            if OpenClipboard(WINDOW.load(Ordering::Relaxed) as HWND) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if !opened {
            GlobalFree(handle);
            return Err("Clipboard busy".into());
        }
        if EmptyClipboard() == 0 {
            CloseClipboard();
            GlobalFree(handle);
            return Err("Cannot clear clipboard".into());
        }
        if SetClipboardData(CF_DIB as u32, handle).is_null() {
            CloseClipboard();
            GlobalFree(handle);
            return Err("Cannot copy image to clipboard".into());
        }
        CloseClipboard(); // Windows now owns the moveable memory.
    }
    if !auto {
        return Ok("Photo copied to clipboard (auto-paste is off)");
    }
    std::thread::sleep(Duration::from_millis(120));
    unsafe {
        if GetForegroundWindow().is_null() {
            return Err("No active Windows app; image remains on clipboard".into());
        }
        let keyboard = |vk: u16, release: bool| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if release { KEYEVENTF_KEYUP } else { 0 },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let inputs = [
            keyboard(VK_CONTROL, false),
            keyboard(0x56, false),
            keyboard(0x56, true),
            keyboard(VK_CONTROL, true),
        ];
        if SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<INPUT>() as i32,
        ) != inputs.len() as u32
        {
            return Err("Ctrl+V unavailable; image remains on clipboard".into());
        }
    }
    Ok("Ctrl+V sent to focused PC window")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_dib_matches_bitmap_layout() {
        let image = DynamicImage::new_rgb8(1, 2);
        let dib = bitmap_dib(&image);
        assert_eq!(dib.len(), 48);
        assert_eq!(&dib[0..4], &40u32.to_le_bytes());
        assert_eq!(&dib[14..16], &24u16.to_le_bytes());
    }
}
