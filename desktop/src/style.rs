//! Mise-inspired warm neutral interface built on GPUI Kit's theme system.
//! Apply once at startup so Sidebar, Button, Input and Textarea all share the
//! same high-contrast surface, rather than painting over individual controls.
use gpui_kit::component::theme::{Theme, ThemeMode};
use gpui_kit::*;

pub fn apply(cx: &mut App) {
    Theme::change(ThemeMode::Light, None, cx);
    Theme::update(cx, |theme| {
        let c = &mut theme.colors;
        c.background = rgb(0xf9f9f7).into();
        c.foreground = rgb(0x242526).into();
        c.sidebar = rgb(0xf1f1ef).into();
        c.sidebar_foreground = rgb(0x353637).into();
        c.sidebar_border = rgb(0xe5e5e2).into();
        c.sidebar_accent = rgb(0xe5e5e2).into();
        c.sidebar_accent_foreground = rgb(0x171819).into();
        c.sidebar_primary = rgb(0x212223).into();
        c.sidebar_primary_foreground = rgb(0xffffff).into();
        c.border = rgb(0xe6e6e3).into();
        c.input = rgb(0xddddda).into();
        c.muted = rgb(0xf3f3f1).into();
        c.muted_foreground = rgb(0x777976).into();
        c.secondary = rgb(0xf1f1ef).into();
        c.secondary_foreground = rgb(0x343534).into();
        c.secondary_hover = rgb(0xe9e9e6).into();
        c.secondary_active = rgb(0xdededa).into();
        c.primary = rgb(0x222323).into();
        c.primary_foreground = rgb(0xffffff).into();
        c.primary_hover = rgb(0x393a39).into();
        c.primary_active = rgb(0x141515).into();
        c.button_primary = rgb(0x222323).into();
        c.button_primary_foreground = rgb(0xffffff).into();
        c.button_primary_hover = rgb(0x393a39).into();
        c.button_primary_active = rgb(0x141515).into();
        c.button_secondary = rgb(0xf0f0ee).into();
        c.button_secondary_foreground = rgb(0x343534).into();
        c.button_secondary_hover = rgb(0xe9e9e6).into();
        c.button_secondary_active = rgb(0xdededa).into();
        c.accent = rgb(0xe9e9e6).into();
        c.accent_foreground = rgb(0x242526).into();
        c.popover = rgb(0xffffff).into();
        c.popover_foreground = rgb(0x242526).into();
        c.ring = rgb(0x397ec5).into();
        c.selection = rgb(0xd8e8fa).into();
    });
}
