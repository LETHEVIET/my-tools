//! Cross-thread requests from the lightweight Win32 tray into the GPUI
//! foreground executor. GPUI windows must only be created on its UI thread.
use crate::{hub::Hub, note_window};
use gpui_kit::*;
use std::cell::RefCell;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

#[derive(Clone, Copy)]
enum Request {
    Note(bool),
    Pair,
    SavedChanged,
}

static REQUESTS: OnceLock<Mutex<Vec<Request>>> = OnceLock::new();
fn requests() -> &'static Mutex<Vec<Request>> {
    REQUESTS.get_or_init(|| Mutex::new(Vec::new()))
}

thread_local! {
    static HUB: RefCell<Option<Entity<Hub>>> = const { RefCell::new(None) };
}

pub fn set_hub(handle: (AnyWindowHandle, Entity<Hub>)) {
    HUB.with(|hub| *hub.borrow_mut() = Some(handle.1));
}

pub fn request_note(quick: bool) {
    requests().lock().unwrap().push(Request::Note(quick));
}

pub fn request_pairing() {
    requests().lock().unwrap().push(Request::Pair);
}

pub fn request_saved_changed() {
    requests().lock().unwrap().push(Request::SavedChanged);
}

pub fn start(cx: &mut App) {
    cx.spawn(async |cx| loop {
        cx.background_executor()
            .timer(Duration::from_millis(85))
            .await;
        let requests = std::mem::take(&mut *requests().lock().unwrap());
        if requests.is_empty() {
            continue;
        }
        cx.update(|cx| {
            for request in requests {
                match request {
                    Request::Note(quick) => note_window::open(cx, quick),
                    Request::Pair => HUB.with(|hub| {
                        if let Some(handle) = hub.borrow().as_ref() {
                            handle.update(cx, |view, cx| {
                                view.show_pairing_page(cx);
                            });
                        }
                        crate::win::show_hub();
                    }),
                    Request::SavedChanged => HUB.with(|hub| {
                        if let Some(handle) = hub.borrow().as_ref() {
                            handle.update(cx, |view, cx| {
                                view.refresh_after_note_save(cx);
                            });
                        }
                    }),
                }
            }
        });
    })
    .detach();
}
