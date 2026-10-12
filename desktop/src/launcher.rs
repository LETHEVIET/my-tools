//! A small GPUI Kit command palette, opened globally with Ctrl+Shift+Space.
//! Search is local: SQLite FTS5 for notes and the Quick Tasks table for tasks.
use crate::{quick_notes_store, tasks, ui_bridge, win};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    button::Button,
    input::{Input, InputEvent, InputState},
    ActiveTheme as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::cell::RefCell;

actions!(
    my_tools_launcher,
    [ChooseResult, DismissLauncher, NextResult, PreviousResult]
);
const CONTEXT: &str = "MyToolsLauncher";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", ChooseResult, Some(CONTEXT)),
        KeyBinding::new("escape", DismissLauncher, Some(CONTEXT)),
        KeyBinding::new("down", NextResult, Some(CONTEXT)),
        KeyBinding::new("up", PreviousResult, Some(CONTEXT)),
    ]);
}

thread_local! {
    static CURRENT: RefCell<Option<AnyWindowHandle>> = const { RefCell::new(None) };
}

#[derive(Clone)]
enum Action {
    NewNote,
    Prompt,
    Tasks,
    Saved,
    Pair,
    Dashboard,
    Note(i64),
    Task(i64),
}

#[derive(Clone)]
struct ResultItem {
    label: String,
    detail: String,
    action: Action,
}

pub fn open(cx: &mut App) {
    if let Some(handle) = CURRENT.with(|current| *current.borrow()) {
        if handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
        {
            return;
        }
    }
    let options = WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some("Quick Launcher - My Tools".into()),
            ..Default::default()
        }),
        window_bounds: Some(WindowBounds::centered(size(px(590.), px(410.)), cx)),
        ..WindowOptions::default()
    };
    match open_window(options, cx, |window, cx| {
        window.on_window_should_close(cx, |_, _| {
            CURRENT.with(|current| *current.borrow_mut() = None);
            true
        });
        cx.new(|cx| Palette::new(window, cx))
    }) {
        Ok((window, _)) => CURRENT.with(|current| *current.borrow_mut() = Some(window)),
        Err(error) => win::error_dialog(&format!("Cannot open Quick Launcher: {error}")),
    };
}

fn find(query: &str) -> Vec<ResultItem> {
    let query_lower = query.to_lowercase();
    let actions = [
        (
            "New Quick Note",
            "Capture an idea · Ctrl+Alt+N",
            Action::NewNote,
        ),
        (
            "Prompt Notes",
            "Compose and paste into an AI app",
            Action::Prompt,
        ),
        (
            "Quick Tasks",
            "Add, complete, or reschedule tasks",
            Action::Tasks,
        ),
        ("Saved Notes", "Search or edit your notes", Action::Saved),
        (
            "Pair iPhone",
            "Show pairing QR inside My Tools",
            Action::Pair,
        ),
        (
            "Open My Tools",
            "Show the main dashboard",
            Action::Dashboard,
        ),
    ];
    let mut found: Vec<ResultItem> = actions
        .into_iter()
        .filter(|(label, detail, _)| {
            query_lower.is_empty()
                || label.to_lowercase().contains(&query_lower)
                || detail.to_lowercase().contains(&query_lower)
        })
        .map(|(label, detail, action)| ResultItem {
            label: label.into(),
            detail: detail.into(),
            action,
        })
        .collect();
    if !query.trim().is_empty() {
        if let Ok(notes) = quick_notes_store::list(query, 5) {
            found.extend(notes.into_iter().map(|note| ResultItem {
                label: note.title,
                detail: format!("Saved {} note · Open in My Tools", note.kind),
                action: Action::Note(note.id),
            }));
        }
        if let Ok(matching) = tasks::list(tasks::TaskFilter::All, query) {
            found.extend(matching.into_iter().take(5).map(|task| ResultItem {
                label: task.title,
                detail: if task.completed_at.is_some() {
                    "Completed task".into()
                } else {
                    format!("Task · {}", tasks::local_due_label(task.due_at))
                },
                action: Action::Task(task.id),
            }));
        }
    }
    found.truncate(12);
    found
}

pub struct Palette {
    query: Entity<InputState>,
    results: Vec<ResultItem>,
    selected: usize,
    _query_subscription: Subscription,
}

impl Palette {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx));
        query.read(cx).focus_handle(cx).focus(window, cx);
        let subscription = cx.subscribe(&query, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let value = this.query.read(cx).value().to_string();
                this.results = find(&value);
                this.selected = 0;
                cx.notify();
            }
        });
        Self {
            query,
            results: find(""),
            selected: 0,
            _query_subscription: subscription,
        }
    }

    fn close(window: &mut Window) {
        CURRENT.with(|current| *current.borrow_mut() = None);
        window.remove_window();
    }

    fn run(&mut self, action: Action, window: &mut Window) {
        Self::close(window);
        match action {
            Action::NewNote => ui_bridge::request_note(true),
            Action::Prompt => ui_bridge::request_note(false),
            Action::Tasks => ui_bridge::request_tasks(),
            Action::Task(id) => ui_bridge::request_task(id),
            Action::Saved => ui_bridge::request_hub(),
            Action::Pair => ui_bridge::request_pairing(),
            Action::Dashboard => ui_bridge::request_hub(),
            Action::Note(id) => ui_bridge::request_saved_note(id),
        }
    }

    fn choose(&mut self, window: &mut Window) {
        if let Some(item) = self.results.get(self.selected).cloned() {
            self.run(item.action, window);
        }
    }

    fn move_selection(&mut self, step: isize, cx: &mut Context<Self>) {
        if !self.results.is_empty() {
            let length = self.results.len() as isize;
            self.selected = (self.selected as isize + step).rem_euclid(length) as usize;
            cx.notify();
        }
    }
}

impl Render for Palette {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let selected = self.selected;
        let entries = self.results.clone();
        div()
            .size_full()
            .bg(rgb(0xffffff))
            .text_color(theme.foreground)
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .key_context(CONTEXT)
            .on_action(cx.listener(|this, _: &ChooseResult, window, _| this.choose(window)))
            .on_action(cx.listener(|_, _: &DismissLauncher, window, _| Self::close(window)))
            .on_action(cx.listener(|this, _: &NextResult, _, cx| this.move_selection(1, cx)))
            .on_action(cx.listener(|this, _: &PreviousResult, _, cx| this.move_selection(-1, cx)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_lg().font_bold().child("Quick Launcher"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("Ctrl+Shift+Space"),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Find tools, saved notes, and tasks"),
            )
            .child(Input::new(&self.query).aria_label("Search tools, tasks, and saved notes"))
            .child(
                div()
                    .id("launcher-results")
                    .flex_1()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .when(entries.is_empty(), |view| {
                        view.child(
                            div()
                                .p_3()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child("No matches. Try a note keyword or tool name."),
                        )
                    })
                    .children(entries.into_iter().enumerate().map(|(index, item)| {
                        let action = item.action.clone();
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .rounded_lg()
                            .p_2()
                            .border_1()
                            .border_color(theme.border)
                            .bg(if index == selected {
                                theme.sidebar_accent
                            } else {
                                theme.background
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().font_bold().text_sm().child(item.label))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(theme.muted_foreground)
                                            .child(item.detail),
                                    ),
                            )
                            .child(
                                Button::new(format!("launcher-run-{index}"))
                                    .label("Open")
                                    .on_click(cx.listener(move |this, _, window, _| {
                                        this.run(action.clone(), window)
                                    })),
                            )
                    })),
            )
            .child(
                div().text_xs().text_color(theme.muted_foreground).child(
                    "↑ / ↓ to select · Enter to open · Esc to close · Search stays on this PC",
                ),
            )
    }
}
