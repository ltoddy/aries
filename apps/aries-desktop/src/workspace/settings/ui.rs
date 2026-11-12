use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::OpenSettings;
use crate::workspace::WorkspaceView;
use crate::workspace::settings::SendKey;
use crate::workspace::settings::palette::Palette;

#[derive(IntoElement)]
pub struct SettingsButton {
    host: Option<WeakEntity<WorkspaceView>>,
    palette: Palette,
}

impl SettingsButton {
    pub fn new(host: Option<WeakEntity<WorkspaceView>>, palette: Palette) -> Self {
        Self { host, palette }
    }
}

impl RenderOnce for SettingsButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = self.palette;
        let host = self.host;
        let error = host
            .as_ref()
            .and_then(|host| host.upgrade())
            .and_then(|host| host.read(cx).settings_state().read(cx).error().map(str::to_owned));
        Button::new("settings")
            .ghost()
            .small()
            .icon(Icon::new(IconName::Settings).small())
            .size_7()
            .disabled(host.is_none())
            .text_color(if error.is_some() { p.danger() } else { p.muted() })
            .tooltip_with_action(
                error.map(|error| format!("设置\n{error}")).unwrap_or_else(|| "设置".into()),
                &OpenSettings,
                None,
            )
            .accessibility_label("打开设置")
            .on_click(move |_, window, cx| {
                if let Some(host) = &host {
                    let _ = host.update(cx, |this, cx| this.open_settings(window, cx));
                }
            })
    }
}

pub fn render_shortcuts(send_key: SendKey, cx: &App) -> impl IntoElement {
    let modifier = if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" };
    let mut sessions = vec![
        ("关闭当前标签", format!("{modifier} + W")),
        ("下一个标签", "Ctrl + Tab".into()),
        ("上一个标签", "Ctrl + Shift + Tab".into()),
        ("下一个标签（替代）", format!("{modifier} + Shift + ]")),
        ("上一个标签（替代）", format!("{modifier} + Shift + [")),
        ("退出应用", format!("{modifier} + Q")),
    ];
    if cfg!(target_os = "macos") {
        sessions.insert(0, ("新建会话", "⌘ + N".into()));
        sessions.insert(1, ("显示/隐藏终端", "⌘ + F".into()));
        sessions.push(("打开设置", "⌘ + ,".into()));
    }
    let groups = [
        ("会话与标签", sessions),
        (
            "消息导航",
            vec![
                ("上一条用户消息", format!("{modifier} + Alt + ↑")),
                ("下一条用户消息", format!("{modifier} + Alt + ↓")),
            ],
        ),
        (
            "输入与弹框",
            vec![
                ("发送消息", send_key.send_label().into()),
                ("输入框换行", send_key.newline_label().into()),
                ("关闭弹框", "Esc".into()),
            ],
        ),
    ];
    div().flex().flex_col().gap_6().text_color(cx.theme().foreground).children(
        groups.into_iter().map(|(title, rows)| {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(title))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .rounded(cx.theme().radius_tokens().lg)
                        .bg(cx.theme().group_box)
                        .children(rows.into_iter().enumerate().map(|(index, (label, keys))| {
                            div()
                                .mx_2p5()
                                .py_2()
                                .min_h(rems(2.375))
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_4()
                                .text_sm()
                                .when(index > 0, |row| {
                                    row.border_t_1().border_color(cx.theme().border.opacity(0.5))
                                })
                                .child(div().flex_1().min_w_0().child(label))
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .text_color(cx.theme().muted_foreground)
                                        .font_family(cx.theme().mono_font_family.clone())
                                        .child(keys),
                                )
                        })),
                )
        }),
    )
}
