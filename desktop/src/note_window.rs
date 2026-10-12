//! Small, themed GPUI Kit note composer. No native Win32 EDIT controls or
//! separate process: Prompt Notes and Quick Note share a Textarea component.
use crate::{notes, quick_notes_store};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::{InputEvent, Textarea, TextareaState},
    ActiveTheme as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::RefCell;
use std::process::Command;

actions!(my_tools_note, [SaveNote, PasteNote, DismissNote]);
const NOTE_CONTEXT: &str = "MyToolsNote";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-s", SaveNote, Some(NOTE_CONTEXT)),
        KeyBinding::new("ctrl-enter", PasteNote, Some(NOTE_CONTEXT)),
        KeyBinding::new("escape", DismissNote, Some(NOTE_CONTEXT)),
    ]);
}

thread_local! {
    static CURRENT: RefCell<Option<(bool, AnyWindowHandle)>> = const { RefCell::new(None) };
}

pub fn open(cx: &mut App, quick: bool) {
    let existing = CURRENT.with(|current| current.borrow().clone());
    if let Some((mode, handle)) = existing {
        if mode == quick {
            if handle
                .update(cx, |_, window, _| window.activate_window())
                .is_ok()
            {
                return;
            }
        } else {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
            CURRENT.with(|current| *current.borrow_mut() = None);
        }
    }
    let options = WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(
                if quick {
                    "Quick Note - My Tools"
                } else {
                    "Prompt Notes - My Tools"
                }
                .into(),
            ),
            ..Default::default()
        }),
        window_bounds: Some(WindowBounds::centered(size(px(560.), px(405.)), cx)),
        ..WindowOptions::default()
    };
    match open_window(options, cx, |window, cx| {
        window.on_window_should_close(cx, |_, _| {
            CURRENT.with(|handle| *handle.borrow_mut() = None);
            true
        });
        cx.new(|cx| NoteWindow::new(quick, window, cx))
    }) {
        Ok(handle) => {
            CURRENT.with(|current| *current.borrow_mut() = Some((quick, handle.0)));
        }
        Err(e) => crate::win::error_dialog(&format!("Cannot open note editor: {e}")),
    }
}

pub struct NoteWindow {
    quick: bool,
    editor: Entity<TextareaState>,
    _changed: Subscription,
    status: String,
}

impl NoteWindow {
    fn new(quick: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text = notes::load_gpui_draft(quick);
        let editor = cx.new(|cx| TextareaState::new(window, cx).default_value(text));
        // A global shortcut should put the caret into the note immediately.
        let focus = editor.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        let _changed = cx.subscribe(&editor, move |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let text = this.editor.read(cx).value().to_string();
                if let Err(error) = notes::save_gpui_draft(this.quick, &text) {
                    this.status = error;
                    cx.notify();
                }
            }
        });
        Self {
            quick,
            editor,
            _changed,
            status: String::new(),
        }
    }

    fn content(&self, cx: &App) -> String {
        self.editor.read(cx).value().to_string()
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let content = self.content(cx);
        // Ctrl+S always creates a new entry on the Saved Notes page.
        // Prompt Notes additionally retains its reusable working draft.
        let result = (|| {
            if !self.quick {
                notes::save_gpui_draft(false, &content)?;
            }
            quick_notes_store::save_kind(&content, if self.quick { "quick" } else { "prompt" })?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                if self.quick {
                    // Saving a Quick Note creates a new note, clears its draft
                    // and closes the popup (Ctrl+S and Save both do this).
                    if let Err(error) = notes::save_gpui_draft(true, "") {
                        self.status = error;
                        cx.notify();
                        return;
                    }
                }
                crate::ui_bridge::request_saved_changed();
                CURRENT.with(|handle| *handle.borrow_mut() = None);
                window.remove_window();
            }
            Err(error) => {
                self.status = error;
                cx.notify();
            }
        }
    }

    fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let content = self.content(cx);
        if let Err(error) = notes::save_gpui_draft(false, &content) {
            self.status = error;
            cx.notify();
            return;
        }
        // Closing the GPUI popup restores the original Win32 focus target.
        CURRENT.with(|handle| *handle.borrow_mut() = None);
        window.remove_window();
        if let Err(message) = notes::paste_gpui_text(&content) {
            crate::win::info_dialog(&message);
        }
    }

    fn open_folder(&mut self, cx: &mut Context<Self>) {
        match quick_notes_store::data_folder().and_then(|folder| {
            std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
            Command::new("explorer.exe")
                .arg(folder)
                .spawn()
                .map_err(|e| e.to_string())?;
            Ok(())
        }) {
            Err(error) => {
                self.status = error;
                cx.notify();
            }
            Ok(()) => {}
        }
    }

    fn copy_only(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let content = self.content(cx);
        if let Err(error) =
            notes::save_gpui_draft(false, &content).and_then(|_| notes::copy_gpui_text(&content))
        {
            self.status = error;
            cx.notify();
            return;
        }
        CURRENT.with(|handle| *handle.borrow_mut() = None);
        window.remove_window();
    }
}

impl Render for NoteWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .size_full()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(0xffffff))
            .text_color(theme.foreground)
            .key_context(NOTE_CONTEXT)
            .on_action(cx.listener(|this, _: &SaveNote, window, cx| this.save(window, cx)))
            .on_action(cx.listener(|this, _: &PasteNote, window, cx| {
                if this.quick {
                    this.save(window, cx)
                } else {
                    this.paste(window, cx)
                }
            }))
            .on_action(cx.listener(|_, _: &DismissNote, window, _| {
                CURRENT.with(|handle| *handle.borrow_mut() = None);
                window.remove_window();
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pb_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_lg().font_bold().child(if self.quick {
                                "Quick Note"
                            } else {
                                "Prompt Notes"
                            }))
                            .child(div().text_xs().text_color(theme.muted_foreground).child(
                                if self.quick {
                                    "Capture an idea. Save it locally, nothing gets pasted."
                                } else {
                                    "Compose your prompt here before pasting it into AI."
                                },
                            )),
                    )
                    .child(div().text_xs().text_color(theme.muted_foreground).child(
                        if self.quick {
                            "Ctrl+Alt+N"
                        } else {
                            "Ctrl+Alt+P"
                        },
                    )),
            )
            .child(
                div().flex_1().min_h_0().child(
                    Textarea::new(&self.editor)
                        .h(relative(1.))
                        .aria_label("Note editor"),
                ),
            )
            .when(!self.status.is_empty(), |this| {
                this.child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(self.status.clone()),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pt_3()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(div().text_xs().text_color(theme.muted_foreground).child(
                        if self.quick {
                            "Ctrl+S saves and closes"
                        } else {
                            "Ctrl+Enter pastes · Ctrl+S saves to Notes"
                        },
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .when(!self.quick, |this| {
                                this.child(
                                    Button::new("note-save-to-notes")
                                        .label("Save to Notes")
                                        .on_click(
                                            cx.listener(|this, _, window, cx| {
                                                this.save(window, cx)
                                            }),
                                        ),
                                )
                            })
                            .child(
                                Button::new("note-secondary")
                                    .label(if self.quick {
                                        "Data folder"
                                    } else {
                                        "Copy only"
                                    })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if this.quick {
                                            this.open_folder(cx)
                                        } else {
                                            this.copy_only(window, cx)
                                        }
                                    })),
                            )
                            .child(
                                Button::new("note-primary")
                                    .primary()
                                    .label(if self.quick {
                                        "Save Note"
                                    } else {
                                        "Paste to AI"
                                    })
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        if this.quick {
                                            this.save(window, cx)
                                        } else {
                                            this.paste(window, cx)
                                        }
                                    })),
                            ),
                    ),
            )
    }
}
