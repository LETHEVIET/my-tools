//! My Tools application shell. Uses GPUI Kit's Sidebar, Button, Badge, Icon and
//! built-in system theme rather than custom styling or a second UI runtime.
use super::{install, open_notes, quick_notes_store, receiver_online, win};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    badge::Badge,
    button::{Button, ButtonVariants},
    sidebar::{Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
    ActiveTheme as _, Icon, IconName,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use qrcode::{Color, QrCode};
use std::path::PathBuf;
use std::process::Command;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Home,
    Photo,
    Prompt,
    Quick,
    Saved,
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
            Self::Settings => "Settings & updates",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::Home => "A focused place for the little tools you use every day.",
            Self::Photo => "Capture on iPhone. Paste directly into your Windows AI prompt.",
            Self::Prompt => "Compose first, then paste only when you're ready.",
            Self::Quick => "Write something down without sending or pasting it anywhere.",
            Self::Saved => "Your local collection of saved quick notes.",
            Self::Settings => "Keep My Tools running, paired and up to date.",
        }
    }
}

pub(super) struct Hub {
    page: Page,
    status: String,
    notes: Vec<(PathBuf, String)>,
    selected: Option<PathBuf>,
    selected_content: String,
    collapsed: bool,
}

impl Hub {
    pub(super) fn new() -> Self {
        Self {
            page: Page::Home,
            status: "Ready. Closing the window keeps your tools running in the tray.".into(),
            notes: quick_notes_store::recent(200),
            selected: None,
            selected_content: String::new(),
            collapsed: false,
        }
    }

    fn navigate(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        if page == Page::Saved {
            self.refresh_saved();
        }
        cx.notify();
    }

    fn refresh_saved(&mut self) {
        self.notes = quick_notes_store::recent(200);
        if let Some(path) = &self.selected {
            if !self.notes.iter().any(|(item, _)| item == path) {
                self.selected = None;
                self.selected_content.clear();
            }
        }
    }

    pub(super) fn refresh_after_note_save(&mut self, cx: &mut Context<Self>) {
        self.refresh_saved();
        cx.notify();
    }

    fn pick_note(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                self.selected_content = contents;
                self.selected = Some(path);
                self.status = "Note loaded from your PC. No clipboard changes.".into();
            }
            Err(error) => self.status = format!("Cannot open saved note: {error}"),
        }
        cx.notify();
    }

    fn open_notes_folder(&mut self, cx: &mut Context<Self>) {
        self.status = match quick_notes_store::folder() {
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
            .w(px(212.))
            .collapsible(true)
            .collapsed(collapsed)
            .header(
                SidebarHeader::new().child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .px_2()
                        .py_2()
                        .child(Icon::new(IconName::LayoutDashboard))
                        .when(!collapsed, |this| {
                            this.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().font_bold().text_lg().child("My Tools"))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("PERSONAL WORKSPACE"),
                                    ),
                            )
                        }),
                ),
            )
            .child(
                SidebarGroup::new("Workspace").child(
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
                            SidebarMenuItem::new("Saved Notes")
                                .icon(IconName::FolderOpen)
                                .active(active == Page::Saved)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.navigate(Page::Saved, cx)),
                                ),
                        ),
                ),
            )
            .child(
                SidebarGroup::new("Tools").child(
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
                SidebarGroup::new("Preferences").child(
                    SidebarMenu::new().child(
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
                                    .child("v0.5 · Runs in system tray"),
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
            .bg(theme.secondary)
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

    fn page_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        match self.page {
            Page::Home => {
                div().flex().flex_col().gap_3()
                    .child(div().flex().justify_between().items_center()
                        .child(div().text_sm().text_color(theme.muted_foreground).child("Available tools"))
                        .child(Badge::new().count(3)))
                    .child(self.tool_card("Snap to Paste Image",
                        "Capture a photo on iPhone and paste it into the active AI prompt.", "iPhone → Windows",
                        IconName::FileText,
                        Button::new("home-pair").primary().label("Pair iPhone")
                            .on_click(cx.listener(|this, _, _, cx| this.show_pairing(cx))), cx))
                    .child(self.tool_card("Prompt Notes",
                            "Prepare long prompts before pasting.", "Ctrl+Alt+P",
                            IconName::FileText,
                            Button::new("home-prompt").label("Open Prompt Notes")
                                .on_click(cx.listener(|this, _, _, cx| this.open_prompt(cx))), cx))
                    .child(self.tool_card("Quick Note",
                            "Save ideas without copying or pasting.", "Ctrl+Alt+N",
                            IconName::FileText,
                            Button::new("home-quick").label("Write a note")
                                .on_click(cx.listener(|this, _, _, cx| this.open_quick(cx))), cx))
                    .child(div().p_4().rounded_xl().border_1().border_color(theme.border)
                        .flex().items_center().justify_between()
                        .child(div().flex().flex_col().gap_1()
                            .child(div().font_bold().child("Your saved notes"))
                            .child(div().text_sm().text_color(theme.muted_foreground)
                                .child(format!("{} local notes, never uploaded", self.notes.len()))))
                        .child(Button::new("home-saved").label("Browse notes")
                            .on_click(cx.listener(|this, _, _, cx| this.navigate(Page::Saved, cx)))))
                    .into_any_element()
            }
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
                let selected = self.selected.clone();
                let preview = self.selected_content.clone();
                div().flex().flex_col().gap_4()
                    .child(div().flex().items_center().justify_between()
                        .child(div().flex().items_center().gap_2()
                            .child(Icon::new(IconName::FolderOpen))
                            .child(div().text_sm().text_color(theme.muted_foreground)
                                .child(format!("{} notes stored on this computer", self.notes.len()))))
                        .child(div().flex().gap_2()
                            .child(Button::new("saved-refresh").label("Refresh")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.refresh_saved(); cx.notify();
                                })))
                            .child(Button::new("saved-folder").label("Open folder")
                                .on_click(cx.listener(|this, _, _, cx| this.open_notes_folder(cx))))))
                    .child(div().flex().gap_3().min_h(px(300.))
                        .child(div().w(px(208.)).flex_shrink_0().rounded_lg().border_1()
                            .border_color(theme.border).p_3().gap_2().flex().flex_col()
                            .child(div().text_sm().font_bold().child("Recent notes"))
                            .child(div().id("saved-notes-scroll").flex_1().overflow_y_scroll()
                                .gap_2().flex().flex_col()
                                .when(note_items.is_empty(), |this| this.child(
                                    div().text_sm().text_color(theme.muted_foreground)
                                        .child("Nothing saved yet. Use Ctrl+Alt+N to add your first note.")))
                                .children(note_items.into_iter().enumerate().map(|(ix, (path, title))| {
                                    let chosen = selected.as_ref() == Some(&path);
                                    Button::new(format!("saved-entry-{ix}"))
                                        .label(title)
                                        .when(chosen, |this| this.primary())
                                        .on_click(cx.listener(move |this, _, _, cx| this.pick_note(path.clone(), cx)))
                                        .into_any_element()
                                }))))
                        .child(div().flex_1().min_w_0().rounded_lg().border_1()
                            .border_color(theme.border).p_3().flex().flex_col().gap_3()
                            .child(div().font_bold().child("Note preview"))
                            .child(div().text_sm().text_color(theme.muted_foreground)
                                .child("Select a note on the left to read it. Notes remain on your PC."))
                            .child(div().id("saved-note-preview").flex_1().overflow_y_scroll()
                                .child(if selected.is_some() { preview } else {
                                    "Choose a saved note to see its contents here.".into()
                                }))))
                    .into_any_element()
            }
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
            .flex()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(sidebar)
            .child(
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
                            .border_b_1()
                            .border_color(theme.border)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().text_xl().font_bold().child(self.page.heading()))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(theme.muted_foreground)
                                            .child(self.page.subtitle()),
                                    ),
                            )
                            .child(Badge::new().dot().color(if receiver_online() {
                                theme.success
                            } else {
                                theme.muted_foreground
                            })),
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
            )
    }
}
