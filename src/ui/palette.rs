use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Hsla, WindowAppearance, rgb};

use crate::preferences::Appearance;

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
                0x202020, 0x181818, 0x2b2b2b, 0x333333, 0xf3f3f3, 0xa3a3a3, 0x383838, 0x303030,
                0xfca5a5,
            ],
            ThemeMode::Light => [
                0xffffff, 0xfafafa, 0xffffff, 0xf0f0f0, 0x202020, 0x737373, 0xe3e3e3, 0xf0f0f0,
                0xb91c1c,
            ],
        };
        let [background, sidebar, input, bubble, foreground, muted, border, hover, danger] =
            colors.map(|color| rgb(color).into());
        Self { background, sidebar, input, bubble, foreground, muted, border, hover, danger }
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
    let font_size = Theme::global(cx).font_size;
    let mono_font_size = Theme::global(cx).mono_font_size;
    Theme::change(mode, None, cx);
    let p = Palette::new(mode);
    Theme::update(cx, |theme| {
        theme.font_size = font_size;
        theme.mono_font_size = mono_font_size;
        theme.colors.background = p.background();
        theme.colors.foreground = p.foreground();
        theme.colors.primary = p.foreground();
        theme.colors.primary_foreground = p.background();
        theme.colors.primary_hover = p.muted();
        theme.colors.secondary = p.bubble();
        theme.colors.secondary_foreground = p.foreground();
        theme.colors.muted = p.bubble();
        theme.colors.muted_foreground = p.muted();
        theme.colors.accent = p.hover();
        theme.colors.accent_foreground = p.foreground();
        theme.colors.border = p.border();
        theme.colors.input = p.border();
        theme.colors.ring = p.muted();
    });
}
