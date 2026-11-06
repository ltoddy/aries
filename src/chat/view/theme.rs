use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::select::Select;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::view::ChatView;
use crate::chat::view::palette::Palette;
use crate::chat::view::tabs::TabsView;
use crate::preferences::{Appearance, SendKey};

impl ChatView {
    pub fn render_settings_button(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
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
    pub fn open_settings(&self, window: &mut Window, cx: &mut Context<Self>) {
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
                    let send_key = this.settings.send_key();
                    let send_keys = [SendKey::Enter, SendKey::ShiftEnter];
                    let send_key_view = view.clone();
                    let font_size = this.settings.font_size();
                    let font_select = this.font_select.clone();
                    let error = this.settings_error.clone();
                    let saving = this.settings_saving;
                    let path =
                        this.preferences_path.as_ref().map(|path| path.display().to_string());
                    let appearances = [Appearance::System, Appearance::Light, Appearance::Dark];
                    let theme_view = view.clone();
                    let retry = view.clone();
                    let font_smaller = view.clone();
                    let font_larger = view.clone();
                    let font_reset = view.clone();
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
                            div().flex().flex_col().gap_2().child("字体").child(
                                Select::new(&font_select)
                                    .id("settings-font-family")
                                    .accessibility_label("字体")
                                    .search_placeholder("搜索字体")
                                    .menu_max_h(rems(16.))
                                    .w_full(),
                            ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .child("字号")
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Button::new("settings-font-smaller")
                                                .icon(IconName::Minus)
                                                .small()
                                                .disabled(font_size <= 12.0)
                                                .accessibility_label("减小字号")
                                                .on_click(move |_, _, cx| {
                                                    let _ = font_smaller.update(cx, |this, cx| {
                                                        this.change_font_size(-1, cx)
                                                    });
                                                }),
                                        )
                                        .child(
                                            Button::new("settings-font-reset")
                                                .label(format!("{font_size} px"))
                                                .small()
                                                .tooltip("恢复默认字号（16 px）")
                                                .accessibility_label("恢复默认字号")
                                                .on_click(move |_, _, cx| {
                                                    let _ = font_reset.update(cx, |this, cx| {
                                                        this.change_font_size(0, cx)
                                                    });
                                                }),
                                        )
                                        .child(
                                            Button::new("settings-font-larger")
                                                .icon(IconName::Plus)
                                                .small()
                                                .disabled(font_size >= 24.0)
                                                .accessibility_label("增大字号")
                                                .on_click(move |_, _, cx| {
                                                    let _ = font_larger.update(cx, |this, cx| {
                                                        this.change_font_size(1, cx)
                                                    });
                                                }),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("修改即时生效并自动保存。字号范围为 12–24 px。"),
                        )
                        .child(div().font_weight(FontWeight::MEDIUM).child("消息输入"))
                        .child(
                            div().flex().flex_col().gap_2().child("发送消息按键").child(
                                RadioGroup::new("settings-send-key")
                                    .selected_index(
                                        send_keys.iter().position(|key| *key == send_key),
                                    )
                                    .children([
                                        Radio::new("send-key-enter")
                                            .label("Enter 发送，Shift + Enter 换行"),
                                        Radio::new("send-key-shift-enter")
                                            .label("Shift + Enter 发送，Enter 换行"),
                                    ])
                                    .on_change(move |index, _, cx| {
                                        if let Some(send_key) = send_keys.get(*index) {
                                            let _ = send_key_view.update(cx, |this, cx| {
                                                this.set_send_key(*send_key, cx);
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
                                .child("快捷键")
                                .child(
                                    Button::new("shortcut-help")
                                        .label("快捷键…")
                                        .small()
                                        .accessibility_label("查看快捷键说明")
                                        .on_click(move |_, window, cx| {
                                            open_shortcut_help(send_key, window, cx)
                                        }),
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

fn open_shortcut_help(send_key: SendKey, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _, cx| {
        dialog.title("快捷键").centered().width(cx.theme().font_size * 30.).content(
            move |content, _, cx| {
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
                        "输入与弹框",
                        vec![
                            ("发送消息", send_key.send_label().into()),
                            ("输入框换行", send_key.newline_label().into()),
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
