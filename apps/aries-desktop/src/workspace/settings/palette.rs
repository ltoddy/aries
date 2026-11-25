use std::sync::Arc;

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Hsla, WindowAppearance, rgb};

use crate::workspace::settings::Appearance;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    background: Hsla,
    sidebar: Hsla,
    input: Hsla,
    bubble: Hsla,
    foreground: Hsla,
    muted: Hsla,
    border: Hsla,
    hover: Hsla,
    danger: Hsla,
    primary: Hsla,
    primary_hover: Hsla,
    selection: Hsla,
    text_selection: Hsla,
    input_border: Hsla,
    success: Hsla,
    warning: Hsla,
    info: Hsla,
    link: Hsla,
}

impl Palette {
    pub fn background(&self) -> Hsla {
        self.background
    }
    pub fn sidebar(&self) -> Hsla {
        self.sidebar
    }
    pub fn input(&self) -> Hsla {
        self.input
    }
    pub fn bubble(&self) -> Hsla {
        self.bubble
    }
    pub fn foreground(&self) -> Hsla {
        self.foreground
    }
    pub fn muted(&self) -> Hsla {
        self.muted
    }
    pub fn border(&self) -> Hsla {
        self.border
    }
    pub fn hover(&self) -> Hsla {
        self.hover
    }
    pub fn danger(&self) -> Hsla {
        self.danger
    }
    pub fn new(mode: ThemeMode) -> Self {
        let colors = match mode {
            ThemeMode::Dark => [
                0x303033, 0x252528, 0x3a3a3e, 0x3b3b3f, 0xf0f0f2, 0xb3b3ba, 0x525257, 0xf06c66,
                0x5e9bea, 0x74a9ef, 0x5b5b63, 0x63c27a, 0xeac04a, 0x5aa5f5, 0x78b4f5,
            ],
            ThemeMode::Light => [
                0xfafafa, 0xededf0, 0xf1f1f3, 0xececef, 0x27272a, 0x62626b, 0xd5d5da, 0xcc3d3d,
                0x2f74c0, 0x2866ac, 0xc4c4cc, 0x3a9150, 0x9a7400, 0x2f74c0, 0x246aaa,
            ],
        };
        let [
            background,
            sidebar,
            input,
            bubble,
            foreground,
            muted,
            border,
            danger,
            primary,
            primary_hover,
            input_border,
            success,
            warning,
            info,
            link,
        ] = colors.map(|color| rgb(color).into());
        Self {
            background,
            sidebar,
            input,
            bubble,
            foreground,
            muted,
            border,
            hover: foreground.opacity(0.07),
            danger,
            primary,
            primary_hover,
            selection: foreground.opacity(0.115),
            text_selection: primary.opacity(0.25),
            input_border,
            success,
            warning,
            info,
            link,
        }
    }
}

pub fn apply(appearance: Appearance, cx: &mut App) {
    let native = match appearance {
        Appearance::System => None,
        Appearance::Light => Some(WindowAppearance::Light),
        Appearance::Dark => Some(WindowAppearance::Dark),
    };
    cx.set_window_appearance(native);
    let mode = native.unwrap_or_else(|| cx.window_appearance()).into();
    sync(mode, cx);
}

pub fn sync(mode: ThemeMode, cx: &mut App) {
    let font_family = Theme::global(cx).font_family.clone();
    let font_size = Theme::global(cx).font_size;
    let mono_font_size = Theme::global(cx).mono_font_size;
    Theme::change(mode, None, cx);
    let palette = Palette::new(mode);
    Theme::update(cx, |theme| {
        theme.font_family = font_family;
        theme.font_size = font_size;
        theme.mono_font_size = mono_font_size;
        theme.colors.background = palette.background();
        theme.colors.foreground = palette.foreground();
        theme.colors.primary = palette.primary;
        theme.colors.primary_foreground = rgb(0xffffff).into();
        theme.colors.primary_hover = palette.primary_hover;
        theme.colors.primary_active = palette.primary_hover;
        theme.colors.secondary = palette.bubble();
        theme.colors.secondary_foreground = palette.foreground();
        theme.colors.secondary_hover = palette.input_border;
        theme.colors.secondary_active = palette.selection;
        theme.colors.button = palette.bubble();
        theme.colors.button_foreground = palette.foreground();
        theme.colors.button_hover = palette.hover();
        theme.colors.button_active = palette.selection;
        theme.colors.button_primary = palette.primary;
        theme.colors.button_primary_foreground = theme.colors.primary_foreground;
        theme.colors.button_primary_hover = palette.primary_hover;
        theme.colors.button_primary_active = palette.primary_hover;
        theme.colors.button_secondary = palette.bubble();
        theme.colors.button_secondary_foreground = palette.foreground();
        theme.colors.button_secondary_hover = palette.input_border;
        theme.colors.button_secondary_active = palette.selection;
        theme.colors.muted = palette.bubble();
        theme.colors.muted_foreground = palette.muted();
        theme.colors.accent = palette.hover();
        theme.colors.accent_foreground = palette.foreground();
        theme.colors.border = palette.border();
        theme.colors.input = palette.input_border;
        theme.colors.ring = palette.primary;
        theme.colors.caret = palette.primary;
        theme.colors.selection = palette.text_selection;
        theme.colors.popover = palette.input();
        theme.colors.popover_foreground = palette.foreground();
        theme.colors.group_box = palette.foreground().opacity(0.045);
        theme.colors.group_box_foreground = palette.foreground();
        theme.colors.sidebar = palette.sidebar();
        theme.colors.sidebar_foreground = palette.foreground();
        theme.colors.sidebar_border = palette.border();
        theme.colors.sidebar_accent = palette.selection;
        theme.colors.sidebar_accent_foreground = palette.foreground();
        theme.colors.sidebar_primary = palette.primary;
        theme.colors.sidebar_primary_foreground = theme.colors.primary_foreground;
        theme.colors.list = palette.background();
        theme.colors.list_head = palette.bubble();
        theme.colors.list_even = palette.bubble();
        theme.colors.list_hover = palette.hover();
        theme.colors.list_active = palette.selection;
        theme.colors.list_active_border = palette.selection;
        theme.colors.title_bar = palette.sidebar();
        theme.colors.title_bar_border = palette.border().opacity(0.5);
        theme.colors.danger = palette.danger();
        theme.colors.success = palette.success;
        theme.colors.warning = palette.warning;
        theme.colors.info = palette.info;
        theme.colors.link = palette.link;
        theme.colors.link_hover = palette.primary;
        theme.colors.link_active = palette.primary_hover;
        theme.colors.scrollbar_thumb = palette.muted().opacity(0.4);
        theme.colors.scrollbar_thumb_hover = palette.muted().opacity(0.6);
        let highlight = &mut Arc::make_mut(&mut theme.highlight_theme).style;
        highlight.editor_background = Some(palette.background());
        highlight.editor_foreground = Some(palette.foreground());
        highlight.editor_gutter_background = Some(palette.background());
        highlight.editor_line_number = Some(palette.muted());
        highlight.editor_active_line_number = Some(palette.foreground());
        highlight.editor_active_line = Some(palette.hover());
        highlight.editor_invisible = Some(palette.border());
    });
}
