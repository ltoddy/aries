use crate::workspace::settings::state::*;

impl SettingsState {
    pub fn open_settings(&self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let view = view.clone();
            dialog
                .title(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("设置"))
                .centered()
                .width(cx.theme().font_size * 44.)
                .h(rems(28.))
                .bg(cx.theme().background)
                .rounded(cx.theme().radius_tokens().xl)
                .content(move |content, _, cx| {
                    let Some(owner) = view.upgrade() else {
                        return content;
                    };
                    let this = owner.read(cx);
                    let page = this.page;
                    let appearance = this.settings.appearance();
                    let send_key = this.settings.send_key();
                    let send_keys = [SendKey::Enter, SendKey::CommandEnter];
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
                    let navigation = div().flex().flex_col().gap_1().children(
                        [
                            SettingsPage::Appearance,
                            SettingsPage::MessageInput,
                            SettingsPage::Shortcuts,
                        ]
                        .into_iter()
                        .map(|target| {
                            let view = view.clone();
                            Button::new(target.id())
                                .ghost()
                                .small()
                                .label(target.label())
                                .when(page == target, |button| {
                                    button.font_weight(FontWeight::SEMIBOLD)
                                })
                                .w_full()
                                .justify_start()
                                .selected(page == target)
                                .on_click(move |_, _, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        this.page = target;
                                        cx.notify();
                                    });
                                    cx.refresh_windows();
                                })
                        }),
                    );
                    let page_content = div()
                        .flex()
                        .flex_col()
                        .gap_6()
                        .p_5()
                        .text_sm()
                        .text_color(cx.theme().foreground)
                        .when(page == SettingsPage::Appearance, |content| {
                            content
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div().font_weight(FontWeight::SEMIBOLD).child("主题"),
                                        )
                                        .child(
                                            div()
                                                .p_3()
                                                .rounded(cx.theme().radius_tokens().lg)
                                                .bg(cx.theme().group_box)
                                                .child(
                                                    RadioGroup::new("settings-theme")
                                                        .selected_index(
                                                            appearances.iter().position(|value| {
                                                                *value == appearance
                                                            }),
                                                        )
                                                        .children([
                                                            Radio::new("theme-system")
                                                                .small()
                                                                .label("跟随系统"),
                                                            Radio::new("theme-light")
                                                                .small()
                                                                .label("浅色"),
                                                            Radio::new("theme-dark")
                                                                .small()
                                                                .label("深色"),
                                                        ])
                                                        .on_change(move |index, _, cx| {
                                                            if let Some(appearance) =
                                                                appearances.get(*index)
                                                            {
                                                                let _ = theme_view.update(
                                                                    cx,
                                                                    |this, cx| {
                                                                        this.set_appearance(
                                                                            *appearance,
                                                                            cx,
                                                                        )
                                                                    },
                                                                );
                                                            }
                                                        }),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div().font_weight(FontWeight::SEMIBOLD).child("文字"),
                                        )
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .rounded(cx.theme().radius_tokens().lg)
                                                .bg(cx.theme().group_box)
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap_4()
                                                        .mx_2p5()
                                                        .py_2()
                                                        .min_h(rems(2.375))
                                                        .child(
                                                            div()
                                                                .w(rems(3.))
                                                                .flex_shrink_0()
                                                                .child("字体"),
                                                        )
                                                        .child(
                                                            div().flex_1().min_w_0().child(
                                                                Select::new(&font_select)
                                                                    .small()
                                                                    .id("settings-font-family")
                                                                    .accessibility_label("字体")
                                                                    .search_placeholder("搜索字体")
                                                                    .menu_max_h(rems(16.))
                                                                    .w_full(),
                                                            ),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap_4()
                                                        .mx_2p5()
                                                        .py_2()
                                                        .min_h(rems(2.375))
                                                        .border_t_1()
                                                        .border_color(
                                                            cx.theme().border.opacity(0.5),
                                                        )
                                                        .child(
                                                            div()
                                                                .w(rems(3.))
                                                                .flex_shrink_0()
                                                                .child("字号"),
                                                        )
                                                        .child(
                                                            div()
                                                                .flex()
                                                                .flex_1()
                                                                .min_w_0()
                                                                .items_center()
                                                                .justify_end()
                                                                .gap_1()
                                                                .child(
                                                                    Button::new(
                                                                        "settings-font-smaller",
                                                                    )
                                                                    .icon(IconName::Minus)
                                                                    .small()
                                                                    .disabled(font_size <= 12.0)
                                                                    .accessibility_label("减小字号")
                                                                    .on_click(move |_, _, cx| {
                                                                        let _ = font_smaller.update(cx, |this, cx| this.change_font_size(-1, cx));
                                                                    }),
                                                                )
                                                                .child(
                                                                    Button::new(
                                                                        "settings-font-reset",
                                                                    )
                                                                    .label(format!(
                                                                        "{font_size} px"
                                                                    ))
                                                                    .small()
                                                                    .w(rems(4.5))
                                                                    .tooltip(
                                                                        "恢复默认字号（16 px）",
                                                                    )
                                                                    .accessibility_label(
                                                                        "恢复默认字号",
                                                                    )
                                                                    .on_click(move |_, _, cx| {
                                                                        let _ = font_reset.update(cx, |this, cx| this.change_font_size(0, cx));
                                                                    }),
                                                                )
                                                                .child(
                                                                    Button::new(
                                                                        "settings-font-larger",
                                                                    )
                                                                    .icon(IconName::Plus)
                                                                    .small()
                                                                    .disabled(font_size >= 24.0)
                                                                    .accessibility_label("增大字号")
                                                                    .on_click(move |_, _, cx| {
                                                                        let _ = font_larger.update(cx, |this, cx| this.change_font_size(1, cx));
                                                                    }),
                                                                ),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child("字号范围为 12–24 px。"),
                                        ),
                                )
                        })
                        .when(page == SettingsPage::MessageInput, |content| {
                            content.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child("发送消息按键"),
                                    )
                                    .child(
                                        div()
                                            .p_3()
                                            .rounded(cx.theme().radius_tokens().lg)
                                            .bg(cx.theme().group_box)
                                            .child(
                                                RadioGroup::new("settings-send-key")
                                                    .selected_index(
                                                        send_keys
                                                            .iter()
                                                            .position(|key| *key == send_key),
                                                    )
                                                    .children([
                                                        Radio::new("send-key-enter").small().label(
                                                            format!(
                                                                "Enter 发送，{} 换行",
                                                                SendKey::Enter.newline_label(),
                                                            ),
                                                        ),
                                                        Radio::new("send-key-command-enter")
                                                            .small()
                                                            .label(format!(
                                                                "{} 发送，Enter 换行",
                                                                SendKey::CommandEnter.send_label(),
                                                            )),
                                                    ])
                                                    .on_change(move |index, _, cx| {
                                                        if let Some(send_key) =
                                                            send_keys.get(*index)
                                                        {
                                                            let _ = send_key_view.update(
                                                                cx,
                                                                |this, cx| {
                                                                    this.set_send_key(
                                                                        *send_key, cx,
                                                                    );
                                                                },
                                                            );
                                                        }
                                                    }),
                                            ),
                                    ),
                            )
                        })
                        .when(page == SettingsPage::Shortcuts, |content| {
                            content.child(render_shortcuts(send_key, cx))
                        });
                    content
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .gap_3()
                        .child(
                            div()
                                .flex()
                                .items_stretch()
                                .flex_1()
                                .min_h_0()
                                .rounded(cx.theme().radius_tokens().lg)
                                .bg(cx.theme().background)
                                .child(
                                    div()
                                        .w(rems(9.))
                                        .min_h_0()
                                        .flex_shrink_0()
                                        .bg(cx.theme().sidebar)
                                        .rounded_l(cx.theme().radius_tokens().lg)
                                        .child(div().p_2().child(navigation))
                                        .overflow_y_scrollbar()
                                        .id("settings-navigation"),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .min_h_0()
                                        .bg(cx.theme().background)
                                        .rounded_r(cx.theme().radius_tokens().lg)
                                        .child(page_content)
                                        .overflow_y_scrollbar()
                                        .id(page.id()),
                                ),
                        )
                        .child(
                            div()
                                .id("settings-save-status")
                                .h_auto()
                                .max_h(rems(8.))
                                .flex_shrink_0()
                                .overflow_y_scrollbar()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child("修改即时生效并自动保存。"),
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
                                                        div()
                                                            .text_sm()
                                                            .text_color(cx.theme().danger)
                                                            .child(error),
                                                    )
                                                    .child(
                                                        Button::new("settings-retry")
                                                            .label("重试保存")
                                                            .small()
                                                            .disabled(saving)
                                                            .on_click(move |_, _, cx| {
                                                                let _ =
                                                                    retry.update(cx, |this, cx| {
                                                                        this.save_settings(cx)
                                                                    });
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
                                        }),
                                ),
                        )
                })
        });
    }
}
