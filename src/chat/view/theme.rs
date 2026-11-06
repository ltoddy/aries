use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::view::ChatView;
use crate::chat::view::palette::Palette;
use crate::chat::view::tabs::TabsView;
use crate::preferences::Appearance;

impl ChatView {
    pub(super) fn render_settings_button(
        &self,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let host = self.tab_host.clone();
        let error = host
            .as_ref()
            .and_then(|host| host.upgrade())
            .and_then(|host| host.read(cx).settings_error.clone());
        Button::new("settings")
            .ghost()
            .icon(Icon::new(IconName::Settings).size(rems(1.125)))
            .size_8()
            .disabled(host.is_none())
            .text_color(if error.is_some() { p.danger() } else { p.muted() })
            .tooltip(error.map(|error| format!("设置\n{error}")).unwrap_or_else(|| "设置".into()))
            .accessibility_label("打开设置")
            .on_click(move |_, window, cx| {
                if let Some(host) = &host {
                    let _ = host.update(cx, |this, cx| this.open_settings(window, cx));
                }
            })
    }
}

impl TabsView {
    pub(super) fn open_settings(&self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let view = view.clone();
            dialog.title("设置").centered().width(cx.theme().font_size * 28.).content(
                move |content, _, cx| {
                    let Some(owner) = view.upgrade() else {
                        return content;
                    };
                    let this = owner.read(cx);
                    let appearance = this.settings.appearance();
                    let zoom = this.settings.zoom();
                    let error = this.settings_error.clone();
                    let saving = this.settings_saving;
                    let path =
                        this.preferences_path.as_ref().map(|path| path.display().to_string());
                    let appearances = [Appearance::System, Appearance::Light, Appearance::Dark];
                    let theme_view = view.clone();
                    let smaller = view.clone();
                    let larger = view.clone();
                    let reset = view.clone();
                    let retry = view.clone();
                    content
                        .gap_4()
                        .child(div().font_weight(FontWeight::MEDIUM).child("外观"))
                        .child(
                            div().flex().flex_col().gap_2().child("主题").child(
                                RadioGroup::new("settings-theme")
                                    .selected_index(
                                        appearances.iter().position(|value| *value == appearance),
                                    )
                                    .children([
                                        Radio::new("theme-system").label("跟随系统"),
                                        Radio::new("theme-light").label("浅色"),
                                        Radio::new("theme-dark").label("深色"),
                                    ])
                                    .on_change(move |index, _, cx| {
                                        if let Some(appearance) = appearances.get(*index) {
                                            let _ = theme_view.update(cx, |this, cx| {
                                                this.set_appearance(*appearance, cx)
                                            });
                                        }
                                    }),
                            ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .child("界面缩放")
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Button::new("settings-zoom-out")
                                                .icon(IconName::Minus)
                                                .small()
                                                .disabled(zoom <= 0.8)
                                                .accessibility_label("缩小界面")
                                                .on_click(move |_, _, cx| {
                                                    let _ = smaller
                                                        .update(cx, |this, cx| this.zoom(-1, cx));
                                                }),
                                        )
                                        .child(
                                            Button::new("settings-zoom-reset")
                                                .label(format!("{:.0}%", zoom * 100.))
                                                .small()
                                                .tooltip("恢复默认大小")
                                                .on_click(move |_, _, cx| {
                                                    let _ = reset
                                                        .update(cx, |this, cx| this.zoom(0, cx));
                                                }),
                                        )
                                        .child(
                                            Button::new("settings-zoom-in")
                                                .icon(IconName::Plus)
                                                .small()
                                                .disabled(zoom >= 1.5)
                                                .accessibility_label("放大界面")
                                                .on_click(move |_, _, cx| {
                                                    let _ = larger
                                                        .update(cx, |this, cx| this.zoom(1, cx));
                                                }),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("修改即时生效并自动保存；缩放范围为 80%–150%。"),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .child("快捷键")
                                .child(
                                    Button::new("shortcut-help")
                                        .label("快捷键…")
                                        .small()
                                        .accessibility_label("查看快捷键说明")
                                        .on_click(|_, window, cx| open_shortcut_help(window, cx)),
                                ),
                        )
                        .when(saving, |content| {
                            content.child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("正在保存…"),
                            )
                        })
                        .when_some(error, |content, error| {
                            content.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div().text_sm().text_color(cx.theme().danger).child(error),
                                    )
                                    .child(
                                        Button::new("settings-retry")
                                            .label("重试保存")
                                            .small()
                                            .on_click(move |_, _, cx| {
                                                let _ = retry
                                                    .update(cx, |this, cx| this.save_settings(cx));
                                            }),
                                    ),
                            )
                        })
                        .when_some(path, |content, path| {
                            content.child(
                                div()
                                    .text_xs()
                                    .whitespace_normal()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("保存位置：{path}")),
                            )
                        })
                },
            )
        });
    }
}

fn open_shortcut_help(window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, |dialog, _, cx| {
        dialog.title("快捷键").centered().width(cx.theme().font_size * 30.).content(
            |content, _, cx| {
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
                        "界面缩放",
                        vec![
                            ("放大", format!("{modifier} + + / {modifier} + =")),
                            ("缩小", format!("{modifier} + −")),
                            ("恢复默认大小", format!("{modifier} + 0")),
                        ],
                    ),
                    (
                        "输入与弹框",
                        vec![
                            ("发送消息", "Enter".into()),
                            ("输入框换行", "Shift + Enter".into()),
                            ("关闭弹框", "Esc".into()),
                        ],
                    ),
                ];
                content.gap_4().children(groups.into_iter().map(|(title, rows)| {
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(div().font_weight(FontWeight::MEDIUM).child(title))
                        .children(rows.into_iter().map(|(label, keys)| {
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_4()
                                .text_sm()
                                .child(div().flex_1().min_w_0().child(label))
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .font_family(cx.theme().mono_font_family.clone())
                                        .child(keys),
                                )
                        }))
                }))
            },
        )
    });
}
