//! Prompt Notes: a tiny, persistent Win32 scratchpad opened by a global hotkey.
//! No embedded browser, graphics framework, or separate background process.
use std::fs;
use std::mem::{size_of, zeroed};
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{GlobalFree, HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    GetStockObject, GetSysColorBrush, COLOR_BTNFACE, DEFAULT_GUI_FONT,
};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, VK_CONTROL, VK_ESCAPE, VK_RETURN, VK_S,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetForegroundWindow, GetSystemMetrics, GetWindowTextLengthW,
    GetWindowTextW, IsWindow, KillTimer, MessageBoxW, MoveWindow, RegisterClassW, SendMessageW,
    SetForegroundWindow, SetTimer, SetWindowTextW, ShowWindow, BN_CLICKED, EN_CHANGE,
    ES_AUTOVSCROLL, ES_MULTILINE, ES_WANTRETURN, LBN_DBLCLK, LBS_NOTIFY, LB_ADDSTRING,
    LB_GETCURSEL, LB_RESETCONTENT, MB_ICONWARNING, MB_OK, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE,
    SW_RESTORE, SW_SHOW, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_KEYDOWN, WM_SETFOCUS, WM_SETFONT,
    WM_SIZE, WM_TIMER, WNDCLASSW, WS_CHILD, WS_CLIPCHILDREN, WS_EX_CLIENTEDGE, WS_EX_TOOLWINDOW,
    WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};

use crate::quick_notes_store;
use crate::win;

const POPUP_CLASS: &str = "MyToolsPromptNotesPopup";
const EDIT_ID: usize = 3101;
const PASTE_ID: usize = 3102;
const COPY_ID: usize = 3103;
const CLOSE_ID: usize = 3104;
const TIMER_SAVE: usize = 3105;
const SAVED_ID: usize = 3107;
const MAX_CHARS: usize = 500_000;

static POPUP: AtomicIsize = AtomicIsize::new(0);
static EDIT: AtomicIsize = AtomicIsize::new(0);
static TARGET: AtomicIsize = AtomicIsize::new(0);
static QUICK_MODE: AtomicBool = AtomicBool::new(false);
static SAVED_LIST: AtomicIsize = AtomicIsize::new(0);
static SAVED_PATHS: OnceLock<Mutex<Vec<i64>>> = OnceLock::new();
// The GPUI desktop app registers this handler. The standalone Rust test
// receiver can still use the legacy Win32 notes popup.
static GPUI_OPEN: OnceLock<fn(bool)> = OnceLock::new();

pub fn register_gpui_open(handler: fn(bool)) {
    let _ = GPUI_OPEN.set(handler);
}

fn is_quick() -> bool {
    QUICK_MODE.load(Ordering::Relaxed)
}
fn saved_paths() -> &'static Mutex<Vec<i64>> {
    SAVED_PATHS.get_or_init(|| Mutex::new(Vec::new()))
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn draft_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| {
            PathBuf::from(home).join("MyTools").join(if is_quick() {
                "quick-note-draft.txt"
            } else {
                "prompt-notes.txt"
            })
        })
}

fn load_draft() -> String {
    draft_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .unwrap_or_default()
}

fn save_text(text: &str) -> Result<(), String> {
    let path = draft_path().ok_or("Cannot locate Windows user profile")?;
    save_text_to(&path, text)
}

fn save_text_to(path: &std::path::Path, text: &str) -> Result<(), String> {
    let directory = path.parent().ok_or("Missing drafts folder")?;
    fs::create_dir_all(directory).map_err(|e| format!("Could not create drafts folder: {e}"))?;
    // Keep an intact backup until the new text has been safely moved into place.
    let temp = path.with_extension("txt.tmp");
    fs::write(&temp, text.as_bytes()).map_err(|e| format!("Could not save draft: {e}"))?;
    let backup = path.with_extension("txt.bak");
    if path.exists() {
        if backup.exists() {
            fs::remove_file(&backup).map_err(|e| format!("Could not replace old backup: {e}"))?;
        }
        fs::rename(path, &backup).map_err(|e| format!("Could not back up draft: {e}"))?;
    }
    if let Err(e) = fs::rename(&temp, path) {
        if backup.exists() {
            let _ = fs::rename(&backup, path);
        }
        return Err(format!("Could not finalize draft: {e}"));
    }
    if backup.exists() {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

fn gpui_draft_path(quick: bool) -> Result<PathBuf, String> {
    let profile = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or("Cannot locate Windows user profile")?;
    Ok(PathBuf::from(profile).join("MyTools").join(if quick {
        "quick-note-draft.txt"
    } else {
        "prompt-notes.txt"
    }))
}

pub fn load_gpui_draft(quick: bool) -> String {
    gpui_draft_path(quick)
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .unwrap_or_default()
}

pub fn save_gpui_draft(quick: bool, text: &str) -> Result<(), String> {
    save_text_to(&gpui_draft_path(quick)?, text)
}

fn current_text() -> String {
    let edit = EDIT.load(Ordering::Relaxed) as HWND;
    if edit.is_null() {
        return String::new();
    }
    unsafe {
        let count = GetWindowTextLengthW(edit).max(0) as usize;
        let mut buffer = vec![0u16; count.min(MAX_CHARS) + 1];
        let copied = GetWindowTextW(edit, buffer.as_mut_ptr(), buffer.len() as i32).max(0) as usize;
        String::from_utf16_lossy(&buffer[..copied])
    }
}

fn save_draft() {
    if let Err(reason) = save_text(&current_text()) {
        win::error_dialog(&reason);
    }
}

pub fn hide() {
    let hwnd = POPUP.load(Ordering::Relaxed) as HWND;
    if hwnd.is_null() {
        return;
    }
    save_draft();
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
}

pub fn shutdown() {
    let hwnd = POPUP.load(Ordering::Relaxed) as HWND;
    if !hwnd.is_null() {
        save_draft();
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
        }
    }
}

pub fn is_popup_foreground() -> bool {
    let popup = POPUP.load(Ordering::Relaxed) as HWND;
    let active = unsafe { GetForegroundWindow() };
    if active.is_null() {
        return false;
    }
    if !popup.is_null() && active == popup {
        return true;
    }
    // The unified desktop app uses GPUI Kit windows rather than the legacy
    // Win32 popup. Never auto-paste an incoming iPhone image into a note.
    let mut title = [0u16; 96];
    let len = unsafe { GetWindowTextW(active, title.as_mut_ptr(), title.len() as i32) };
    if len <= 0 {
        return false;
    }
    matches!(
        String::from_utf16_lossy(&title[..len as usize]).as_str(),
        "Quick Note - My Tools" | "Prompt Notes - My Tools"
    )
}

pub fn show(tray: HWND, from_shortcut: bool, quick: bool) {
    let previous = unsafe { GetForegroundWindow() };
    let mut popup = POPUP.load(Ordering::Relaxed) as HWND;
    // If opened from a text prompt via Ctrl+Alt+P, remember that window so the
    // text can be pasted back safely after the popup steals keyboard focus.
    if from_shortcut && previous != tray && previous != popup && !previous.is_null() {
        TARGET.store(previous as isize, Ordering::Relaxed);
    } else if !from_shortcut {
        TARGET.store(0, Ordering::Relaxed);
    }
    if let Some(open) = GPUI_OPEN.get() {
        open(quick);
        return;
    }
    let was_quick = is_quick();
    if was_quick != quick && !popup.is_null() {
        save_draft();
    }
    QUICK_MODE.store(quick, Ordering::Relaxed);
    unsafe {
        if popup.is_null() {
            popup = create_popup();
            if popup.is_null() {
                win::error_dialog("Unable to create the Prompt Notes window.");
                return;
            }
        }
        if was_quick != quick {
            SetWindowTextW(
                EDIT.load(Ordering::Relaxed) as HWND,
                wide(&load_draft()).as_ptr(),
            );
        }
        update_mode(popup);
        ShowWindow(popup, SW_RESTORE);
        SetForegroundWindow(popup);
        let edit = EDIT.load(Ordering::Relaxed) as HWND;
        if !edit.is_null() {
            SetFocus(edit);
        }
    }
}

pub fn copy_gpui_text(text: &str) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("Write a prompt before pasting.".into());
    }
    let encoded = wide(text);
    let size = encoded.len() * size_of::<u16>();
    unsafe {
        let data = GlobalAlloc(GMEM_MOVEABLE, size);
        if data.is_null() {
            return Err("Cannot allocate clipboard text".into());
        }
        let memory = GlobalLock(data);
        if memory.is_null() {
            GlobalFree(data);
            return Err("Cannot lock clipboard memory".into());
        }
        std::ptr::copy_nonoverlapping(encoded.as_ptr().cast::<u8>(), memory.cast(), size);
        GlobalUnlock(data);
        if OpenClipboard(win::clipboard_owner()) == 0 {
            GlobalFree(data);
            return Err("Clipboard is busy".into());
        }
        if EmptyClipboard() == 0 || SetClipboardData(CF_UNICODETEXT as u32, data).is_null() {
            CloseClipboard();
            GlobalFree(data);
            return Err("Could not copy prompt".into());
        }
        CloseClipboard();
    }
    Ok(())
}

/// Paste a GPUI editor's content to the application that held focus when
/// Ctrl+Alt+P was pressed. Clipboard fallback remains available if Windows
/// refuses to reactivate that window.
pub fn paste_gpui_text(text: &str) -> Result<(), String> {
    copy_gpui_text(text)?;
    unsafe {
        let target = TARGET.load(Ordering::Relaxed) as HWND;
        if target.is_null() || IsWindow(target) == 0 {
            return Err("Copied to clipboard; focus the AI input and press Ctrl+V.".into());
        }
        ShowWindow(target, SW_SHOW);
        SetForegroundWindow(target);
        std::thread::sleep(std::time::Duration::from_millis(90));
        if GetForegroundWindow() != target {
            return Err(
                "Copied to clipboard; Windows couldn't restore the previous app. Paste manually."
                    .into(),
            );
        }
        if !win::send_paste_shortcut() {
            return Err("Copied to clipboard; Ctrl+V was blocked. Paste manually.".into());
        }
    }
    Ok(())
}

fn refresh_saved_notes() {
    let list = SAVED_LIST.load(Ordering::Relaxed) as HWND;
    if list.is_null() {
        return;
    }
    let notes = quick_notes_store::list("", 15).unwrap_or_default();
    let mut paths = saved_paths().lock().unwrap();
    paths.clear();
    unsafe {
        SendMessageW(list, LB_RESETCONTENT, 0, 0);
        for note in notes {
            SendMessageW(list, LB_ADDSTRING, 0, wide(&note.title).as_ptr() as isize);
            paths.push(note.id);
        }
    }
}

fn open_saved_note() {
    let list = SAVED_LIST.load(Ordering::Relaxed) as HWND;
    let index = unsafe { SendMessageW(list, LB_GETCURSEL, 0, 0) } as i32;
    if index < 0 {
        return;
    }
    let id = saved_paths().lock().unwrap().get(index as usize).copied();
    if let Some(id) = id {
        if let Ok(Some(note)) = quick_notes_store::get(id) {
            unsafe {
                SetWindowTextW(
                    EDIT.load(Ordering::Relaxed) as HWND,
                    wide(&note.body).as_ptr(),
                );
            }
        }
    }
}

fn save_quick_note() {
    match quick_notes_store::save(&current_text()) {
        Ok(_) => {
            unsafe {
                SetWindowTextW(EDIT.load(Ordering::Relaxed) as HWND, wide("").as_ptr());
            }
            save_draft();
            refresh_saved_notes();
            hide();
        }
        Err(message) => warning(&message),
    }
}

fn open_quick_notes_folder() {
    match quick_notes_store::folder() {
        Ok(folder) => {
            if let Err(error) = fs::create_dir_all(&folder) {
                warning(&error.to_string());
                return;
            }
            if let Err(error) = std::process::Command::new("explorer.exe")
                .arg(folder)
                .spawn()
            {
                warning(&error.to_string());
            }
        }
        Err(error) => warning(&error),
    }
}

fn update_mode(hwnd: HWND) {
    let quick = is_quick();
    unsafe {
        SetWindowTextW(
            hwnd,
            wide(if quick {
                "Quick Note - My Tools"
            } else {
                "Prompt Notes - My Tools"
            })
            .as_ptr(),
        );
        let primary =
            windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, PASTE_ID as i32);
        let secondary =
            windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, COPY_ID as i32);
        let hint = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, 3106);
        SetWindowTextW(
            primary,
            wide(if quick { "Save Note" } else { "Paste to AI" }).as_ptr(),
        );
        SetWindowTextW(
            secondary,
            wide(if quick { "Notes Folder" } else { "Copy only" }).as_ptr(),
        );
        SetWindowTextW(
            hint,
            wide(if quick {
                "Ctrl+S: Save note  |  Double-click a saved note to reopen  |  Esc: Hide"
            } else {
                "Ctrl+Enter: Paste  |  Ctrl+S: Save draft  |  Esc: Hide"
            })
            .as_ptr(),
        );
        let list = SAVED_LIST.load(Ordering::Relaxed) as HWND;
        if !list.is_null() {
            ShowWindow(list, if quick { SW_SHOW } else { SW_HIDE });
        }
        let mut bounds: windows_sys::Win32::Foundation::RECT = zeroed();
        windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut bounds);
        layout(hwnd, bounds.right - bounds.left, bounds.bottom - bounds.top);
    }
    if quick {
        refresh_saved_notes();
    }
}

fn copy_text() -> Result<(), String> {
    let text = current_text();
    if text.trim().is_empty() {
        return Err("Type a prompt before copying.".into());
    }
    let encoded = wide(&text);
    let size = encoded.len() * size_of::<u16>();
    unsafe {
        let handle = GlobalAlloc(GMEM_MOVEABLE, size);
        if handle.is_null() {
            return Err("Not enough memory for the clipboard".into());
        }
        let destination = GlobalLock(handle);
        if destination.is_null() {
            GlobalFree(handle);
            return Err("Clipboard allocation failed".into());
        }
        std::ptr::copy_nonoverlapping(encoded.as_ptr().cast::<u8>(), destination.cast(), size);
        GlobalUnlock(handle);
        let mut opened = false;
        for _ in 0..12 {
            if OpenClipboard(POPUP.load(Ordering::Relaxed) as HWND) != 0 {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        if !opened {
            GlobalFree(handle);
            return Err("Clipboard is busy".into());
        }
        if EmptyClipboard() == 0 || SetClipboardData(CF_UNICODETEXT as u32, handle).is_null() {
            CloseClipboard();
            GlobalFree(handle);
            return Err("Could not copy text to the clipboard".into());
        }
        CloseClipboard(); // Windows owns the memory after SetClipboardData.
    }
    Ok(())
}

fn warning(message: &str) {
    unsafe {
        MessageBoxW(
            POPUP.load(Ordering::Relaxed) as HWND,
            wide(message).as_ptr(),
            wide("Prompt Notes").as_ptr(),
            MB_OK | MB_ICONWARNING,
        );
    }
}

fn copy_only() {
    save_draft();
    match copy_text() {
        Ok(()) => hide(),
        Err(msg) => warning(&msg),
    }
}

pub fn paste_to_previous() {
    save_draft();
    if let Err(msg) = copy_text() {
        warning(&msg);
        return;
    }
    let target = TARGET.load(Ordering::Relaxed) as HWND;
    if target.is_null() || unsafe { IsWindow(target) } == 0 {
        warning("Copied to clipboard. Open Prompt Notes using Ctrl+Alt+P while your AI prompt is focused, then Ctrl+Enter to paste back. For now, focus the prompt and press Ctrl+V.");
        hide();
        return;
    }
    hide();
    unsafe {
        ShowWindow(target, SW_SHOW);
        SetForegroundWindow(target);
    }
    std::thread::sleep(std::time::Duration::from_millis(90));
    if unsafe { GetForegroundWindow() } != target {
        warning("Prompt copied, but Windows could not restore the previous window. Focus your AI prompt and press Ctrl+V.");
        return;
    }
    if !win::send_paste_shortcut() {
        warning("Prompt copied, but Ctrl+V was blocked. Paste manually using Ctrl+V.");
    }
}

pub fn handle_key(hwnd: HWND, message: u32, key: usize) -> bool {
    let popup = POPUP.load(Ordering::Relaxed) as HWND;
    if popup.is_null()
        || (hwnd != popup && hwnd != EDIT.load(Ordering::Relaxed) as HWND)
        || message != WM_KEYDOWN
    {
        return false;
    }
    match key as u16 {
        VK_ESCAPE => {
            hide();
            true
        }
        VK_RETURN if unsafe { GetKeyState(VK_CONTROL as i32) } < 0 => {
            if is_quick() {
                save_quick_note()
            } else {
                paste_to_previous()
            }
            true
        }
        VK_S if unsafe { GetKeyState(VK_CONTROL as i32) } < 0 => {
            if is_quick() {
                save_quick_note()
            } else {
                save_draft()
            }
            true
        }
        _ => false,
    }
}

fn layout(hwnd: HWND, width: i32, height: i32) {
    unsafe {
        let edit = EDIT.load(Ordering::Relaxed) as HWND;
        if edit.is_null() {
            return;
        }
        let quick = is_quick();
        let edit_bottom = if quick { height - 222 } else { height - 112 };
        MoveWindow(edit, 14, 14, (width - 28).max(1), edit_bottom.max(90), 1);
        let list = SAVED_LIST.load(Ordering::Relaxed) as HWND;
        if !list.is_null() && quick {
            MoveWindow(
                list,
                14,
                (height - 195).max(105),
                (width - 28).max(1),
                108,
                1,
            );
        }
        for (id, index) in [(PASTE_ID, 0), (COPY_ID, 1), (CLOSE_ID, 2)] {
            let child = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, id as i32);
            MoveWindow(child, 14 + index * 126, height - 82, 118, 34, 1);
        }
        let hint = windows_sys::Win32::UI::WindowsAndMessaging::GetDlgItem(hwnd, 3106);
        MoveWindow(hint, 14, height - 40, width - 28, 22, 1);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = wparam & 0xffff;
            let notification = ((wparam >> 16) & 0xffff) as u32;
            if id == EDIT_ID && notification == EN_CHANGE {
                SetTimer(hwnd, TIMER_SAVE, 700, None);
            } else if id == SAVED_ID && notification == LBN_DBLCLK {
                open_saved_note();
            } else if notification == BN_CLICKED {
                match id {
                    PASTE_ID => {
                        if is_quick() {
                            save_quick_note()
                        } else {
                            paste_to_previous()
                        }
                    }
                    COPY_ID => {
                        if is_quick() {
                            open_quick_notes_folder()
                        } else {
                            copy_only()
                        }
                    }
                    CLOSE_ID => hide(),
                    _ => {}
                }
            }
            0
        }
        WM_TIMER if wparam == TIMER_SAVE => {
            KillTimer(hwnd, TIMER_SAVE);
            save_draft();
            0
        }
        WM_SIZE => {
            layout(
                hwnd,
                (lparam & 0xffff) as i32,
                ((lparam >> 16) & 0xffff) as i32,
            );
            0
        }
        WM_SETFOCUS => {
            SetFocus(EDIT.load(Ordering::Relaxed) as HWND);
            0
        }
        WM_CLOSE => {
            hide();
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, TIMER_SAVE);
            POPUP.store(0, Ordering::Relaxed);
            EDIT.store(0, Ordering::Relaxed);
            SAVED_LIST.store(0, Ordering::Relaxed);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn create_popup() -> HWND {
    let class_name = wide(POPUP_CLASS);
    let class = WNDCLASSW {
        lpfnWndProc: Some(wndproc),
        lpszClassName: class_name.as_ptr(),
        hbrBackground: GetSysColorBrush(COLOR_BTNFACE),
        ..zeroed()
    };
    if RegisterClassW(&class) == 0 {
        return null_mut();
    }
    let width = 570;
    let height = 420;
    let x = (GetSystemMetrics(SM_CXSCREEN) - width) / 2;
    let y = (GetSystemMetrics(SM_CYSCREEN) - height) / 2;
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW,
        class_name.as_ptr(),
        wide("Prompt Notes - My Tools").as_ptr(),
        WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
        x,
        y,
        width,
        height,
        null_mut(),
        null_mut(),
        null_mut(),
        null(),
    );
    if hwnd.is_null() {
        return hwnd;
    }
    POPUP.store(hwnd as isize, Ordering::Relaxed);
    let font = GetStockObject(DEFAULT_GUI_FONT);
    let edit = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        wide("EDIT").as_ptr(),
        null(),
        WS_CHILD
            | WS_VISIBLE
            | WS_VSCROLL
            | WS_TABSTOP
            | ES_MULTILINE as u32
            | ES_AUTOVSCROLL as u32
            | ES_WANTRETURN as u32,
        14,
        14,
        540,
        290,
        hwnd,
        EDIT_ID as *mut _,
        null_mut(),
        null(),
    );
    if edit.is_null() {
        windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd);
        return null_mut();
    }
    EDIT.store(edit as isize, Ordering::Relaxed);
    let saved = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        wide("LISTBOX").as_ptr(),
        null(),
        WS_CHILD | WS_VSCROLL | WS_TABSTOP | LBS_NOTIFY as u32,
        14,
        180,
        540,
        108,
        hwnd,
        SAVED_ID as *mut _,
        null_mut(),
        null(),
    );
    if !saved.is_null() {
        SAVED_LIST.store(saved as isize, Ordering::Relaxed);
        SendMessageW(saved, WM_SETFONT, font as usize, 1);
    }
    SendMessageW(edit, WM_SETFONT, font as usize, 1);
    SendMessageW(
        edit,
        windows_sys::Win32::UI::Controls::EM_SETLIMITTEXT,
        MAX_CHARS,
        0,
    );
    SetWindowTextW(edit, wide(&load_draft()).as_ptr());
    for (id, label) in [
        (PASTE_ID, "Paste to AI"),
        (COPY_ID, "Copy only"),
        (CLOSE_ID, "Close"),
    ] {
        let button = CreateWindowExW(
            0,
            wide("BUTTON").as_ptr(),
            wide(label).as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            0,
            0,
            118,
            34,
            hwnd,
            id as *mut _,
            null_mut(),
            null(),
        );
        SendMessageW(button, WM_SETFONT, font as usize, 1);
    }
    let hint = CreateWindowExW(
        0,
        wide("STATIC").as_ptr(),
        wide("Ctrl+Enter: Paste   |   Ctrl+S: Save   |   Esc: Hide").as_ptr(),
        WS_CHILD | WS_VISIBLE,
        0,
        0,
        500,
        22,
        hwnd,
        3106usize as *mut _,
        null_mut(),
        null(),
    );
    SendMessageW(hint, WM_SETFONT, font as usize, 1);
    layout(hwnd, width, height - 39);
    hwnd
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf16_clipboard_text_retains_unicode() {
        let draft = "Write a long prompt: Ảnh này nói gì? 🚀";
        let encoded = wide(draft);
        assert_eq!(encoded.last(), Some(&0));
        assert_eq!(
            String::from_utf16_lossy(&encoded[..encoded.len() - 1]),
            draft
        );
    }

    #[test]
    fn draft_persists_and_replaces_old_version() {
        let path = std::env::temp_dir()
            .join(format!("my-tools-note-test-{}", std::process::id()))
            .join("draft.txt");
        save_text_to(&path, "A long prompt\nwith many details.").unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "A long prompt\nwith many details."
        );
        save_text_to(&path, "Changed prompt 你好").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "Changed prompt 你好");
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
