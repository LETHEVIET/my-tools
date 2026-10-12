//! My Tools application shell. Uses GPUI Kit's Sidebar, Button, Badge, Icon and
//! built-in system theme rather than custom styling or a second UI runtime.
use super::{install, open_notes, quick_notes_store, receiver_online, tasks, win};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    badge::Badge,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState, Textarea, TextareaState},
    sidebar::{Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    ActiveTheme as _, Icon, IconName,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use qrcode::{Color, QrCode};
use std::process::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Home,
    Photo,
    Prompt,
    Quick,
    Saved,
    Tasks,
    Settings,
}

impl Page {
    fn heading(self) -> &'static str {
        match self {
            Self::Home => "Your workspace",
            Self::Photo => "Snap to Paste Image",
            Self::Prompt => "Prompt Notes",
            Self::Quick => "Quick Note",
            Self::Saved => "Saved Notes",
            Self::Tasks => "Quick Tasks",
            Self::Settings => "Settings & updates",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::Home => "A focused place for the little tools you use every day.",
            Self::Photo => "Capture on iPhone. Paste directly into your Windows AI prompt.",
            Self::Prompt => "Compose first, then paste only when you're ready.",
            Self::Quick => "Write something down without sending or pasting it anywhere.",
            Self::Saved => "Find, review and edit everything you've captured.",
            Self::Tasks => "Capture, finish, and remember what matters.",
            Self::Settings => "Keep My Tools running, paired and up to date.",
        }
    }
}

pub(super) struct Hub {
    page: Page,
    status: String,
    notes: Vec<quick_notes_store::Note>,
    selected: Option<i64>,
    search: Entity<InputState>,
    title: Entity<InputState>,
    tags: Entity<InputState>,
    body: Entity<TextareaState>,
    _search_subscription: Subscription,
    task_title: Entity<InputState>,
    task_due: Entity<InputState>,
    task_filter: tasks::TaskFilter,
    task_items: Vec<tasks::Task>,
    task_selected: Option<i64>,
    day_complete: bool,
    collapsed: bool,
}

impl Hub {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx));
        let title = cx.new(|cx| InputState::new(window, cx));
        let tags = cx.new(|cx| InputState::new(window, cx));
        let body = cx.new(|cx| TextareaState::new(window, cx));
        let task_title = cx.new(|cx| InputState::new(window, cx));
        let task_due = cx.new(|cx| InputState::new(window, cx));
        let _search_subscription = cx.subscribe(&search, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.refresh_saved(cx);
                cx.notify();
            }
        });
        let (notes, status) = match quick_notes_store::list("", 200) {
            Ok(notes) => (
                notes,
                "Ready. Closing the window keeps your tools running in the tray.".into(),
            ),
            Err(err) => (vec![], format!("Cannot open saved notes: {err}")),
        };
        Self {
            page: Page::Home,
            status,
            notes,
            selected: None,
            search,
            title,
            tags,
            body,
            _search_subscription,
            task_title,
            task_due,
            task_filter: tasks::TaskFilter::Today,
            task_items: tasks::list(tasks::TaskFilter::Today, "").unwrap_or_default(),
            task_selected: None,
            day_complete: false,
            collapsed: false,
        }
    }

    fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        self.day_complete = false;
        if page == Page::Saved {
            self.refresh_saved(cx);
        } else if page == Page::Tasks {
            self.refresh_tasks();
        }
        cx.notify();
    }

    pub(super) fn show_tasks_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.navigate(Page::Tasks, cx);
        self.task_title.read(cx).focus_handle(cx).focus(window, cx);
    }

    pub(super) fn show_selected_task(&mut self, id: i64, cx: &mut Context<Self>) {
        self.task_filter = tasks::TaskFilter::All;
        self.navigate(Page::Tasks, cx);
        self.task_selected = Some(id);
        match tasks::get(id) {
            Ok(Some(task)) => {
                self.task_items = vec![task];
                self.status =
                    "Opened task from Quick Launcher. Choose All to see every task.".into();
            }
            Ok(None) => self.status = "This task no longer exists.".into(),
            Err(error) => self.status = error,
        }
        cx.notify();
    }

    pub(super) fn show_saved_note(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        self.page = Page::Saved;
        self.search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.refresh_saved(cx);
        self.pick_note(id, window, cx);
    }

    fn refresh_tasks(&mut self) {
        match tasks::list(self.task_filter, "") {
            Ok(items) => {
                if self
                    .task_selected
                    .is_some_and(|selected| !items.iter().any(|task| task.id == selected))
                {
                    self.task_selected = None;
                }
                if self.task_selected.is_none() {
                    self.task_selected = items.first().map(|task| task.id);
                }
                self.task_items = items;
            }
            Err(error) => self.status = format!("Cannot load tasks: {error}"),
        }
    }

    fn choose_task_filter(&mut self, filter: tasks::TaskFilter, cx: &mut Context<Self>) {
        self.task_filter = filter;
        self.refresh_tasks();
        cx.notify();
    }

    fn select_task(&mut self, id: i64, cx: &mut Context<Self>) {
        self.task_selected = Some(id);
        cx.notify();
    }

    fn preset_tomorrow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.task_due.update(cx, |input, cx| {
            input.set_value(tasks::tomorrow_morning(), window, cx);
        });
    }

    fn clear_due(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.task_due
            .update(cx, |input, cx| input.set_value("", window, cx));
    }

    fn add_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = self.task_title.read(cx).value().to_string();
        let due_text = self.task_due.read(cx).value().to_string();
        let result = tasks::parse_local_due(&due_text).and_then(|due| tasks::add(&title, due));
        match result {
            Ok(id) => {
                self.task_selected = Some(id);
                self.task_title
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.task_due
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.status = if due_text.trim().is_empty() {
                    "Task saved. Add a reminder date if you'd like a Windows notification.".into()
                } else {
                    "Task saved. The Windows tray will remind you when it is due.".into()
                };
                self.task_filter = tasks::TaskFilter::All;
                self.refresh_tasks();
            }
            Err(error) => self.status = error,
        }
        cx.notify();
    }

    fn complete_task(&mut self, id: i64, done: bool, cx: &mut Context<Self>) {
        let result = tasks::set_completed(id, done);
        self.status = match result.as_ref() {
            Ok(()) => if done {
                "Task completed."
            } else {
                "Task reopened."
            }
            .into(),
            Err(error) => error.clone(),
        };
        self.refresh_tasks();
        if result.is_ok()
            && done
            && self.task_filter == tasks::TaskFilter::Today
            && self.task_items.is_empty()
        {
            self.day_complete = true;
        }
        cx.notify();
    }

    fn dismiss_completion(&mut self, cx: &mut Context<Self>) {
        self.day_complete = false;
        cx.notify();
    }

    fn completion_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x15171888))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(440.))
                    .bg(rgb(0xffffff))
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.border)
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .rounded_xl()
                            .bg(rgb(0xe8efea))
                            .p_4()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(div().text_2xl().text_color(rgb(0x448259)).child("✓")),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_bold()
                            .text_center()
                            .child("Today is handled"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_center()
                            .text_color(theme.muted_foreground)
                            .child("You finished the tasks due today. Enjoy the clear space."),
                    )
                    .child(
                        div()
                            .bg(theme.secondary)
                            .rounded_lg()
                            .p_4()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().text_color(theme.success).child("✓"))
                                    .child(div().text_sm().child("No unfinished tasks in Today")),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().text_color(theme.success).child("✓"))
                                    .child(div().text_sm().child("Your updates are saved locally")),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_3()
                            .child(
                                Button::new("day-done-dismiss")
                                    .primary()
                                    .label("Back to tasks")
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.dismiss_completion(cx)),
                                    ),
                            )
                            .child(Button::new("day-done-notes").label("Open notes").on_click(
                                cx.listener(|this, _, _, cx| this.navigate(Page::Saved, cx)),
                            )),
                    ),
            )
    }

    fn reschedule_task(&mut self, id: i64, cx: &mut Context<Self>) {
        let due = self.task_due.read(cx).value().to_string();
        self.status = match tasks::parse_local_due(&due).and_then(|due| tasks::reschedule(id, due))
        {
            Ok(()) => "Reminder updated.".into(),
            Err(error) => error,
        };
        self.refresh_tasks();
        cx.notify();
    }

    fn refresh_saved(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).value().to_string();
        match quick_notes_store::list(&query, 200) {
            Ok(notes) => self.notes = notes,
            Err(err) => {
                self.status = format!("Search failed: {err}");
                self.notes.clear();
            }
        }
        if let Some(id) = self.selected {
            if !self.notes.iter().any(|item| item.id == id) {
                self.selected = None;
            }
        }
    }

    pub(super) fn refresh_after_note_save(&mut self, cx: &mut Context<Self>) {
        self.refresh_saved(cx);
        cx.notify();
    }

    fn pick_note(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        match quick_notes_store::get(id) {
            Ok(Some(note)) => {
                self.title
                    .update(cx, |state, cx| state.set_value(note.title, window, cx));
                self.tags
                    .update(cx, |state, cx| state.set_value(note.tags, window, cx));
                self.body
                    .update(cx, |state, cx| state.set_value(note.body, window, cx));
                self.selected = Some(id);
                self.status = "Note loaded from SQLite. Edit and choose Save changes.".into();
            }
            Ok(None) => self.status = "That note no longer exists.".into(),
            Err(error) => self.status = format!("Cannot open saved note: {error}"),
        }
        cx.notify();
    }

    fn save_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.selected {
            let title = self.title.read(cx).value().to_string();
            let tags = self.tags.read(cx).value().to_string();
            let body = self.body.read(cx).value().to_string();
            self.status = match quick_notes_store::update(id, &title, &body, &tags) {
                Ok(()) => "Changes saved to your local SQLite database.".into(),
                Err(error) => error,
            };
            self.refresh_saved(cx);
            cx.notify();
        }
    }

    fn toggle_pin(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.selected {
            if let Ok(Some(note)) = quick_notes_store::get(id) {
                self.status = match quick_notes_store::set_pinned(id, !note.pinned) {
                    Ok(()) => if note.pinned {
                        "Note unpinned."
                    } else {
                        "Note pinned to the top."
                    }
                    .into(),
                    Err(error) => error,
                };
                self.refresh_saved(cx);
                cx.notify();
            }
        }
    }

    fn open_notes_folder(&mut self, cx: &mut Context<Self>) {
        self.status = match quick_notes_store::data_folder() {
            Ok(folder) => match std::fs::create_dir_all(&folder) {
                Err(error) => format!("Could not create notes directory: {error}"),
                Ok(()) => match Command::new("explorer.exe").arg(folder).spawn() {
                    Ok(_) => "Opened saved notes in File Explorer.".into(),
                    Err(error) => format!("Could not open File Explorer: {error}"),
                },
            },
            Err(error) => error,
        };
        cx.notify();
    }

    fn show_pairing(&mut self, cx: &mut Context<Self>) {
        self.show_pairing_page(cx);
    }

    pub(super) fn show_pairing_page(&mut self, cx: &mut Context<Self>) {
        self.page = Page::Photo;
        self.status = if win::pairing_url().is_some() {
            "Scan the QR with your iPhone camera. Pairing stays inside My Tools.".into()
        } else {
            "The receiver is starting. The pairing code will appear when it is ready.".into()
        };
        cx.notify();
    }

    fn open_prompt(&mut self, cx: &mut Context<Self>) {
        self.status = match open_notes(false) {
            Ok(message) => message.into(),
            Err(error) => error,
        };
        cx.notify();
    }

    fn open_quick(&mut self, cx: &mut Context<Self>) {
        self.status = match open_notes(true) {
            Ok(message) => message.into(),
            Err(error) => error,
        };
        cx.notify();
    }

    fn install(&mut self, cx: &mut Context<Self>) {
        self.status = install::install().unwrap_or_else(|error| error);
        cx.notify();
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.page;
        let collapsed = self.collapsed;
        Sidebar::new("my-tools-navigation")
            .w(px(216.))
            .collapsible(true)
            .collapsed(collapsed)
            .header(
                SidebarHeader::new().child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .py_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(
                                    div()
                                        .size(px(34.))
                                        .rounded_lg()
                                        .bg(rgb(0x252525))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            div()
                                                .text_lg()
                                                .font_bold()
                                                .text_color(rgb(0xffffff))
                                                .child("✦"),
                                        ),
                                )
                                .when(!collapsed, |view| {
                                    view.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .child(div().font_bold().child("My Tools"))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(cx.theme().muted_foreground)
                                                    .child("Personal workspace"),
                                            ),
                                    )
                                }),
                        )
                        .when(!collapsed, |view| {
                            view.child(
                                Button::new("sidebar-search")
                                    .label("Search tools   /")
                                    .on_click(|_, _, _| crate::ui_bridge::request_launcher()),
                            )
                        }),
                ),
            )
            .child(
                SidebarGroup::new("YOUR DAY").child(
                    SidebarMenu::new()
                        .child(
                            SidebarMenuItem::new("Overview")
                                .icon(IconName::LayoutDashboard)
                                .active(active == Page::Home)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.navigate(Page::Home, cx)),
                                ),
                        )
                        .child(
                            SidebarMenuItem::new("Quick Tasks")
                                .icon(IconName::Check)
                                .active(active == Page::Tasks)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.navigate(Page::Tasks, cx)),
                                ),
                        ),
                ),
            )
            .child(
                SidebarGroup::new("CAPTURE").child(
                    SidebarMenu::new()
                        .child(
                            SidebarMenuItem::new("Snap to Paste Image")
                                .icon(IconName::FileText)
                                .active(active == Page::Photo)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.navigate(Page::Photo, cx)),
                                ),
                        )
                        .child(
                            SidebarMenuItem::new("Prompt Notes")
                                .icon(IconName::FileText)
                                .active(active == Page::Prompt)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.navigate(Page::Prompt, cx)),
                                ),
                        )
                        .child(
                            SidebarMenuItem::new("Quick Note")
                                .icon(IconName::FileText)
                                .active(active == Page::Quick)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.navigate(Page::Quick, cx)),
                                ),
                        ),
                ),
            )
            .child(
                SidebarGroup::new("LIBRARY & SETTINGS").child(
                    SidebarMenu::new()
                        .child(
                            SidebarMenuItem::new("Saved Notes")
                                .icon(IconName::FolderOpen)
                                .active(active == Page::Saved)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.navigate(Page::Saved, cx)),
                                ),
                        )
                        .child(
                            SidebarMenuItem::new("Settings")
                                .icon(IconName::Settings)
                                .active(active == Page::Settings)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.navigate(Page::Settings, cx)),
                                ),
                        ),
                ),
            )
            .footer(
                SidebarFooter::new().child(
                    div()
                        .p_3()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .when(!collapsed, |this| {
                            this.child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("LOCAL FIRST  ·  v0.8.1"),
                            )
                        })
                        .child(
                            Button::new("collapse-navigation")
                                .label(if collapsed {
                                    "Expand"
                                } else {
                                    "Collapse sidebar"
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.collapsed = !this.collapsed;
                                    cx.notify();
                                })),
                        ),
                ),
            )
    }

    fn tool_card(
        &self,
        title: &'static str,
        description: &'static str,
        shortcut: &'static str,
        icon: IconName,
        button: Button,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme().clone();
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .p_4()
            .rounded_xl()
            .border_1()
            .border_color(theme.border)
            .bg(rgb(0xffffff))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(Icon::new(icon))
                                    .child(div().font_bold().child(title)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child(description),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(shortcut),
                    ),
            )
            .child(button)
    }

    fn pairing_qr(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let Some(url) = win::pairing_url() else {
            return div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("Starting the receiver…")
                .into_any_element();
        };
        let Ok(qr) = QrCode::new(url.as_bytes()) else {
            return div()
                .text_sm()
                .child("Could not encode pairing QR.")
                .into_any_element();
        };
        let width = qr.width();
        let cells = qr.to_colors();
        let unit = if width > 55 { 4. } else { 5. };
        div()
            .rounded_xl()
            .border_1()
            .border_color(theme.border)
            .bg(rgb(0xffffff))
            .p_4()
            .flex()
            .flex_col()
            .items_center()
            .gap_3()
            .child(div().font_bold().child("Scan with your iPhone"))
            .child(
                div()
                    .p_3()
                    .rounded_lg()
                    .bg(rgb(0xffffff))
                    .flex()
                    .flex_col()
                    .children(cells.chunks(width).map(|line| {
                        div().flex().children(line.iter().map(|cell| {
                            div().w(px(unit)).h(px(unit)).bg(if *cell == Color::Dark {
                                rgb(0x111827)
                            } else {
                                rgb(0xffffff)
                            })
                        }))
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("Opens Snap to Paste Image on your phone. Keep this code private."),
            )
            .into_any_element()
    }

    /// A compact master-detail layout, inspired by the reference review
    /// screen. Navigation stays left; meaningful work stays in the right pane.
    fn task_workspace(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let tasks = self.task_items.clone();
        let selected = self.task_selected;
        let details = selected
            .and_then(|id| tasks.iter().find(|task| task.id == id))
            .cloned();
        let filter = self.task_filter;
        div()
            .flex()
            .gap_4()
            .min_h(px(445.))
            .child(
                div()
                    .w(px(275.))
                    .flex_shrink_0()
                    .bg(rgb(0xffffff))
                    .rounded_xl()
                    .border_1()
                    .border_color(theme.border)
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_3()
                    .child(
                        div().flex().items_center().justify_between()
                            .child(div().font_bold().child("Tasks"))
                            .child(Badge::new().count(tasks.len())),
                    )
                    .child(
                        div().flex().gap_1().flex_wrap().children(
                            [
                                (tasks::TaskFilter::Today, "Today"),
                                (tasks::TaskFilter::Upcoming, "Next"),
                                (tasks::TaskFilter::Completed, "Done"),
                                (tasks::TaskFilter::All, "All"),
                            ].into_iter().map(|(mode, label)| {
                                Button::new(format!("task-filter-{label}"))
                                    .label(label)
                                    .when(filter == mode, |button| button.primary())
                                    .on_click(cx.listener(move |this, _, _, cx| this.choose_task_filter(mode, cx)))
                                    .into_any_element()
                            }),
                        ),
                    )
                    .child(
                        div().id("task-review-list").flex_1().overflow_y_scroll()
                            .flex().flex_col().gap_2()
                            .when(tasks.is_empty(), |view| view.child(
                                div().rounded_lg().p_3().bg(theme.secondary).text_sm()
                                    .text_color(theme.muted_foreground)
                                    .child("All clear here. Add a task or switch views."),
                            ))
                            .children(tasks.into_iter().map(|task| {
                                let id = task.id;
                                let chosen = selected == Some(id);
                                let label = format!(
                                    "{}{}",
                                    if task.completed_at.is_some() { "✓ " } else { "" },
                                    task.title,
                                );
                                div().flex().flex_col().gap_1().p_2()
                                    .rounded_lg().border_1()
                                    .border_color(if chosen { theme.foreground } else { theme.border })
                                    .bg(if chosen { theme.sidebar_accent } else { theme.background })
                                    .child(
                                        Button::new(format!("task-select-{id}")).label(label)
                                            .on_click(cx.listener(move |this, _, _, cx| this.select_task(id, cx))),
                                    )
                                    .child(div().px_2().text_xs().text_color(theme.muted_foreground)
                                        .child(if task.completed_at.is_some() {
                                            "Completed".into()
                                        } else {
                                            tasks::local_due_label(task.due_at)
                                        }))
                            })),
                    )
                    .child(
                        div().border_t_1().border_color(theme.border).pt_3()
                            .child(div().text_xs().text_color(theme.muted_foreground)
                                .child("Tasks stay in your local SQLite database.")),
                    ),
            )
            .child(
                div().flex_1().min_w_0().flex().flex_col().gap_3()
                    .child(
                        div().rounded_xl().border_1().border_color(theme.border)
                            .bg(rgb(0xffffff)).p_4().flex().flex_col().gap_3()
                            .child(
                                div().flex().items_center().justify_between()
                                    .child(div().font_bold().child("Task details"))
                                    .child(
                                        Badge::new().dot().color(
                                            if details.as_ref().is_some_and(|task| task.completed_at.is_some()) {
                                                theme.success
                                            } else {
                                                theme.muted_foreground
                                            },
                                        ),
                                    ),
                            )
                            .child(
                                if let Some(task) = details {
                                    let done = task.completed_at.is_some();
                                    let id = task.id;
                                    div().flex().flex_col().gap_3()
                                        .child(div().text_lg().font_bold().child(task.title))
                                        .child(
                                            div().bg(theme.secondary).rounded_lg().p_3()
                                                .flex().flex_col().gap_1()
                                                .child(div().text_xs().text_color(theme.muted_foreground).child("DUE"))
                                                .child(div().font_bold().child(tasks::local_due_label(task.due_at))),
                                        )
                                        .child(
                                            div().flex().gap_2().flex_wrap()
                                                .child(
                                                    Button::new(format!("task-toggle-{id}"))
                                                        .primary().label(if done { "Reopen task" } else { "Mark complete" })
                                                        .on_click(cx.listener(move |this, _, _, cx| this.complete_task(id, !done, cx))),
                                                )
                                                .when(!done, |view| view.child(
                                                    Button::new(format!("task-due-{id}")).label("Update reminder")
                                                        .on_click(cx.listener(move |this, _, _, cx| this.reschedule_task(id, cx))),
                                                )),
                                        )
                                        .into_any_element()
                                } else {
                                    div().bg(theme.secondary).rounded_lg().p_4()
                                        .text_sm().text_color(theme.muted_foreground)
                                        .child("Select a task on the left to see its details and finish it.")
                                        .into_any_element()
                                },
                            ),
                    )
                    .child(
                        div().rounded_xl().border_1().border_color(theme.border)
                            .bg(rgb(0xffffff)).p_4().flex().flex_col().gap_3()
                            .child(div().font_bold().child("Add a new task"))
                            .child(div().text_xs().text_color(theme.muted_foreground)
                                .child("A quick place to capture what needs doing."))
                            .child(Input::new(&self.task_title).aria_label("Task description"))
                            .child(div().text_xs().text_color(theme.muted_foreground)
                                .child("Optional reminder · YYYY-MM-DD HH:MM"))
                            .child(Input::new(&self.task_due).aria_label("Local due date and time"))
                            .child(
                                div().flex().gap_2().flex_wrap()
                                    .child(Button::new("task-tomorrow").label("Tomorrow at 9")
                                        .on_click(cx.listener(|this, _, window, cx| this.preset_tomorrow(window, cx))))
                                    .child(Button::new("task-no-reminder").label("Clear date")
                                        .on_click(cx.listener(|this, _, window, cx| this.clear_due(window, cx))))
                                    .child(Button::new("task-add").primary().label("Add task")
                                        .on_click(cx.listener(|this, _, window, cx| this.add_task(window, cx)))),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn overview(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        div().flex().flex_col().gap_4()
            .child(
                div().rounded_xl().bg(rgb(0xededeb)).p_5()
                    .flex().items_center().justify_between().gap_4()
                    .child(
                        div().flex_1().min_w_0().flex().flex_col().gap_2()
                            .child(div().text_xs().font_bold().text_color(theme.muted_foreground)
                                .child("YOUR EVERYDAY WORKSPACE"))
                            .child(div().text_xl().font_bold().child("Make room for what matters."))
                            .child(div().text_sm().text_color(theme.muted_foreground)
                                .child("Capture ideas, keep track of tasks, and move between tools without breaking focus."))
                            .child(
                                div().flex().gap_2().pt_2()
                                    .child(Button::new("home-new-note").primary().label("New quick note")
                                        .on_click(cx.listener(|this, _, _, cx| this.open_quick(cx))))
                                    .child(Button::new("home-new-task").label("Open tasks")
                                        .on_click(cx.listener(|this, _, _, cx| this.navigate(Page::Tasks, cx)))),
                            ),
                    )
                    .child(
                        div().size(px(94.)).flex_shrink_0().rounded_xl()
                            .bg(rgb(0xe0e7db)).flex().items_center().justify_center()
                            .child(div().text_color(rgb(0x65846b)).text_3xl().child("✦")),
                    ),
            )
            .child(
                div().flex().gap_3()
                    .child(
                        div().flex_1().min_w_0().rounded_xl().bg(rgb(0xffffff))
                            .border_1().border_color(theme.border).p_4()
                            .flex().flex_col().gap_2()
                            .child(div().text_xs().text_color(theme.muted_foreground).child("SAVED NOTES"))
                            .child(div().text_xl().font_bold().child(format!("{}", self.notes.len())))
                            .child(div().text_xs().text_color(theme.muted_foreground).child("Ideas in your library")),
                    )
                    .child(
                        div().flex_1().min_w_0().rounded_xl().bg(rgb(0xffffff))
                            .border_1().border_color(theme.border).p_4()
                            .flex().flex_col().gap_2()
                            .child(div().text_xs().text_color(theme.muted_foreground).child("QUICK TASKS"))
                            .child(div().text_xl().font_bold().child("Today"))
                            .child(div().text_xs().text_color(theme.muted_foreground).child("See what needs doing")),
                    )
                    .child(
                        div().flex_1().min_w_0().rounded_xl().bg(rgb(0xffffff))
                            .border_1().border_color(theme.border).p_4()
                            .flex().flex_col().gap_2()
                            .child(div().text_xs().text_color(theme.muted_foreground).child("PHONE CONNECTION"))
                            .child(div().text_lg().font_bold().child(
                                if receiver_online() { "Ready" } else { "Offline" }
                            ))
                            .child(div().text_xs().text_color(theme.muted_foreground).child("Snap to Paste Image")),
                    ),
            )
            .child(
                div().flex().items_center().justify_between()
                    .child(div().font_bold().child("Quick actions"))
                    .child(div().text_xs().text_color(theme.muted_foreground)
                        .child("All the little things, in one place")),
            )
            .child(
                div().flex().flex_col().gap_2()
                    .child(self.tool_card("Prompt Notes", "Compose a prompt and paste to your previous app.",
                        "Ctrl+Alt+P", IconName::FileText,
                        Button::new("home-prompt").label("Open")
                            .on_click(cx.listener(|this, _, _, cx| this.open_prompt(cx))), cx))
                    .child(self.tool_card("Saved Notes", "Find and edit your local notes.",
                        "SQLite + search", IconName::FolderOpen,
                        Button::new("home-notes").label("Browse")
                            .on_click(cx.listener(|this, _, _, cx| this.navigate(Page::Saved, cx))), cx))
                    .child(self.tool_card("Quick Launcher", "Search notes, tasks and actions instantly.",
                        "Ctrl+Shift+Space", IconName::LayoutDashboard,
                        Button::new("home-launcher").label("Search")
                            .on_click(|_,_,_| crate::ui_bridge::request_launcher()), cx)),
            )
            .into_any_element()
    }

    fn page_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        match self.page {
            Page::Home => self.overview(cx),
            Page::Photo => div().flex().flex_col().gap_3()
                .child(self.tool_card("Snap to Paste Image",
                    "Your iPhone pairs to this Windows receiver. Focus an AI prompt, then capture an image on your phone.",
                    if receiver_online() { "Receiver online" } else { "Receiver unavailable" },
                    IconName::FileText, Button::new("photo-pair").label("Refresh pairing QR")
                        .on_click(cx.listener(|this, _, _, cx| this.show_pairing(cx))), cx))
                .child(self.pairing_qr(cx))
                .into_any_element(),
            Page::Prompt => self.tool_card("Prompt Notes",
                    "Ctrl+Alt+P opens your long-form prompt scratchpad anywhere. Ctrl+Enter pastes to the previous app.",
                    "Ctrl+Alt+P", IconName::FileText,
                    Button::new("prompt-open").primary().label("Open Prompt Notes")
                        .on_click(cx.listener(|this, _, _, cx| this.open_prompt(cx))), cx)
                .into_any_element(),
            Page::Quick => self.tool_card("Quick Note",
                    "Press Ctrl+Alt+N to capture an idea and Ctrl+S to save a new local note. Nothing gets pasted.",
                    "Ctrl+Alt+N", IconName::FileText,
                    Button::new("quick-open").primary().label("Write a Quick Note")
                        .on_click(cx.listener(|this, _, _, cx| this.open_quick(cx))), cx)
                .into_any_element(),
            Page::Saved => {
                let note_items = self.notes.clone();
                let selected = self.selected;
                let pinned = selected.and_then(|id| self.notes.iter().find(|note| note.id == id))
                    .is_some_and(|note| note.pinned);
                div().flex().flex_col().gap_4()
                    .child(div().flex().items_center().justify_between()
                        .child(div().flex().items_center().gap_2()
                            .child(Icon::new(IconName::FolderOpen))
                            .child(div().text_sm().text_color(theme.muted_foreground)
                                .child(format!("{} matching notes · SQLite + FTS5", self.notes.len()))))
                        .child(div().flex().gap_2()
                            .child(Button::new("saved-new").primary().label("New note")
                                .on_click(cx.listener(|this, _, _, cx| this.open_quick(cx))))
                            .child(Button::new("saved-refresh").label("Refresh")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.refresh_saved(cx); cx.notify();
                                })))
                            .child(Button::new("saved-folder").label("Data folder")
                                .on_click(cx.listener(|this, _, _, cx| this.open_notes_folder(cx))))))
                    .child(div().flex().flex_col().gap_1()
                        .child(div().text_xs().text_color(theme.muted_foreground)
                            .child("Search note titles, text, or tags"))
                        .child(Input::new(&self.search).aria_label("Search saved notes by keywords, body, or tags")))
                    .child(div().flex().gap_3().min_h(px(300.))
                        .child(div().w(px(248.)).flex_shrink_0().rounded_xl().border_1()
                            .border_color(theme.border).bg(rgb(0xffffff)).p_3().gap_2().flex().flex_col()
                            .child(div().text_sm().font_bold().child("Saved notes"))
                            .child(div().id("saved-notes-scroll").flex_1().overflow_y_scroll()
                                .gap_2().flex().flex_col()
                                .when(note_items.is_empty(), |this| this.child(
                                    div().text_sm().text_color(theme.muted_foreground)
                                        .child("No matching notes. Try another search or press Ctrl+Alt+N.")))
                                .children(note_items.into_iter().enumerate().map(|(ix, note)| {
                                    let chosen = selected == Some(note.id);
                                    let id = note.id;
                                    Button::new(format!("saved-entry-{ix}"))
                                        .label(format!("{}{}{}", if note.pinned { "★ " } else { "" },
                                            if note.kind == "prompt" { "Prompt · " } else { "" }, note.title))
                                        .when(chosen, |this| this.primary())
                                        .on_click(cx.listener(move |this, _, window, cx| this.pick_note(id, window, cx)))
                                        .into_any_element()
                                }))))
                        .child(div().flex_1().min_w_0().rounded_xl().border_1()
                            .border_color(theme.border).bg(rgb(0xffffff)).p_4().flex().flex_col().gap_3()
                            .child(div().font_bold().child("Edit note"))
                            .when(selected.is_none(), |this| this.child(
                                div().text_sm().text_color(theme.muted_foreground)
                                    .child("Select a note on the left. Old text files have been imported without deletion.")
                            ))
                            .when(selected.is_some(), |this| this
                                .child(div().text_xs().text_color(theme.muted_foreground).child("Title"))
                                .child(Input::new(&self.title).aria_label("Note title"))
                                .child(div().flex_1().min_h(px(155.)).child(
                                    Textarea::new(&self.body).h(relative(1.)).aria_label("Note body")))
                                .child(div().text_xs().text_color(theme.muted_foreground)
                                    .child("Tags (comma-separated)"))
                                .child(Input::new(&self.tags).aria_label("Tags, separated by commas"))
                                .child(div().flex().gap_2()
                                    .child(Button::new("saved-save").primary().label("Save changes")
                                        .on_click(cx.listener(|this, _, _, cx| this.save_selected(cx))))
                                    .child(Button::new("saved-pin")
                                        .label(if pinned { "Unpin" } else { "Pin to top" })
                                        .on_click(cx.listener(|this, _, _, cx| this.toggle_pin(cx))))))
                        ))
                    .into_any_element()
            }
            Page::Tasks => self.task_workspace(cx),
            Page::Settings => div().flex().flex_col().gap_4()
                .child(div().p_5().rounded_xl().border_1().border_color(theme.border)
                    .flex().flex_col().gap_3()
                    .child(div().font_bold().child("Run in the background"))
                    .child(div().text_sm().text_color(theme.muted_foreground)
                        .child("Closing the window keeps your image receiver and shortcuts active. Use the system tray to reopen or exit."))
                    .child(if install::is_installed() {
                        div().text_sm().child("Installed to your Windows user profile.").into_any_element()
                    } else {
                        Button::new("settings-install").primary().label("Install for Windows startup")
                            .on_click(cx.listener(|this, _, _, cx| this.install(cx))).into_any_element()
                    }))
                .child(div().p_5().rounded_xl().border_1().border_color(theme.border)
                    .flex().flex_col().gap_2()
                    .child(div().font_bold().child("Updates"))
                    .child(div().text_sm().text_color(theme.muted_foreground)
                        .child("Release updates download over public GitHub HTTPS. Automatic updates are controlled from the tray menu.")))
                .into_any_element(),
        }
    }
}

impl Render for Hub {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let sidebar = self.sidebar(cx).into_any_element();
        let content = self.page_content(cx);
        div()
            .size_full()
            .relative()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                div().size_full().flex().child(sidebar).child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .px_5()
                                .py_3()
                                .bg(rgb(0xffffff))
                                .border_b_1()
                                .border_color(theme.border)
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(
                                            div().text_xl().font_bold().child(self.page.heading()),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(theme.muted_foreground)
                                                .child(self.page.subtitle()),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Button::new("top-launcher")
                                                .label("Search tools")
                                                .on_click(|_, _, _| {
                                                    crate::ui_bridge::request_launcher()
                                                }),
                                        )
                                        .child(
                                            Button::new("top-note")
                                                .primary()
                                                .label("+ Note")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.open_quick(cx)
                                                })),
                                        )
                                        .child(Badge::new().dot().color(if receiver_online() {
                                            theme.success
                                        } else {
                                            theme.muted_foreground
                                        })),
                                ),
                        )
                        .child(
                            div()
                                .id("my-tools-main-scroll")
                                .flex_1()
                                .overflow_y_scroll()
                                .p_4()
                                .child(content),
                        )
                        .child(
                            div()
                                .px_4()
                                .py_3()
                                .border_t_1()
                                .border_color(theme.border)
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(self.status.clone()),
                        ),
                ),
            )
            .when(self.day_complete && self.page == Page::Tasks, |root| {
                root.child(self.completion_dialog(cx))
            })
    }
}
