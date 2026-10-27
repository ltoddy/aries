use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::preferences::{Appearance, Preferences};
use crate::ui::ChatView;
use crate::ui::palette::{self, Palette};

impl ChatView {
    pub fn render_theme_button(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let (icon, current, next, target) = match self.appearance {
            Appearance::System => (IconName::Monitor, "跟随系统", Appearance::Light, "Light"),
            Appearance::Light => (IconName::Sun, "Light", Appearance::Dark, "Dark"),
            Appearance::Dark => (IconName::Moon, "Dark", Appearance::System, "跟随系统"),
        };
        let label = format!("当前主题：{current}，点击切换为{target}");
        let tooltip = match &self.preference_error {
            Some(error) => format!("{label}\n{error}"),
            None => label.clone(),
        };
        Button::new("theme-toggle")
            .ghost()
            .icon(Icon::new(icon).size(px(18.)))
            .size_8()
            .text_color(if self.preference_error.is_some() { p.danger() } else { p.muted() })
            .tooltip(tooltip)
            .accessibility_label(label)
            .on_click(cx.listener(move |this, _, _, cx| this.set_appearance(next, cx)))
    }

    fn set_appearance(&mut self, appearance: Appearance, cx: &mut Context<Self>) {
        self.appearance = appearance;
        palette::apply(appearance, cx);
        self.preference_error = match &self.preferences_path {
            Some(path) => Preferences::new(appearance)
                .save(path)
                .err()
                .map(|error| format!("主题已切换，但无法保存偏好：{error}")),
            None => Some("主题已切换；未找到可保存偏好的配置目录".into()),
        };
        cx.notify();
    }
}
