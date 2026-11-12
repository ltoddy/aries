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
    let p = Palette::new(mode);
    Theme::update(cx, |theme| {
        theme.font_family = font_family;
        theme.font_size = font_size;
        theme.mono_font_size = mono_font_size;
        theme.colors.background = p.background();
        theme.colors.foreground = p.foreground();
        theme.colors.primary = p.primary;
        theme.colors.primary_foreground = rgb(0xffffff).into();
        theme.colors.primary_hover = p.primary_hover;
        theme.colors.primary_active = p.primary_hover;
        theme.colors.secondary = p.bubble();
        theme.colors.secondary_foreground = p.foreground();
        theme.colors.secondary_hover = p.input_border;
        theme.colors.secondary_active = p.selection;
        theme.colors.button = p.bubble();
        theme.colors.button_foreground = p.foreground();
        theme.colors.button_hover = p.hover();
        theme.colors.button_active = p.selection;
        theme.colors.button_primary = p.primary;
        theme.colors.button_primary_foreground = theme.colors.primary_foreground;
        theme.colors.button_primary_hover = p.primary_hover;
        theme.colors.button_primary_active = p.primary_hover;
        theme.colors.button_secondary = p.bubble();
        theme.colors.button_secondary_foreground = p.foreground();
        theme.colors.button_secondary_hover = p.input_border;
        theme.colors.button_secondary_active = p.selection;
        theme.colors.muted = p.bubble();
        theme.colors.muted_foreground = p.muted();
        theme.colors.accent = p.hover();
        theme.colors.accent_foreground = p.foreground();
        theme.colors.border = p.border();
        theme.colors.input = p.input_border;
        theme.colors.ring = p.primary;
        theme.colors.caret = p.primary;
        theme.colors.selection = p.text_selection;
        theme.colors.popover = p.input();
        theme.colors.popover_foreground = p.foreground();
        theme.colors.group_box = p.foreground().opacity(0.045);
        theme.colors.group_box_foreground = p.foreground();
        theme.colors.sidebar = p.sidebar();
        theme.colors.sidebar_foreground = p.foreground();
        theme.colors.sidebar_border = p.border();
        theme.colors.sidebar_accent = p.selection;
        theme.colors.sidebar_accent_foreground = p.foreground();
        theme.colors.sidebar_primary = p.primary;
        theme.colors.sidebar_primary_foreground = theme.colors.primary_foreground;
        theme.colors.list = p.background();
        theme.colors.list_head = p.bubble();
        theme.colors.list_even = p.bubble();
        theme.colors.list_hover = p.hover();
        theme.colors.list_active = p.selection;
        theme.colors.list_active_border = p.selection;
        theme.colors.title_bar = p.sidebar();
        theme.colors.title_bar_border = p.border().opacity(0.5);
        theme.colors.danger = p.danger();
        theme.colors.success = p.success;
        theme.colors.warning = p.warning;
        theme.colors.info = p.info;
        theme.colors.link = p.link;
        theme.colors.link_hover = p.primary;
        theme.colors.link_active = p.primary_hover;
        theme.colors.scrollbar_thumb = p.muted().opacity(0.4);
        theme.colors.scrollbar_thumb_hover = p.muted().opacity(0.6);
    });
}
