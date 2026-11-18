use crate::workspace::extensions::*;

impl ExtensionsView {
    pub fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let directory =
            self.directory.as_deref().map(|path| path.display().to_string()).unwrap_or_default();
        div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .gap_3()
            .px_3()
            .py_2()
            .child(
                Button::new("extensions-back")
                    .ghost()
                    .small()
                    .track_focus(&self.back_focus)
                    .icon(IconName::ArrowLeft)
                    .accessibility_label("返回会话")
                    .tooltip("返回会话")
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(ReturnToSessions::new()))),
            )
            .child(div().font_weight(FontWeight::SEMIBOLD).child("Agent 扩展"))
            .child(
                div()
                    .id("extensions-project")
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(directory.clone())
                    .tooltip(move |window, cx| Tooltip::new(directory.clone()).build(window, cx)),
            )
    }

    pub fn render_navigation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(rems(10.5))
            .min_w(rems(8.5))
            .max_w(relative(0.3))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .px_3()
            .py_4()
            .child(div().px_2().text_xs().text_color(cx.theme().muted_foreground).child("本地扩展"))
            .child(div().flex().flex_col().gap_1().children(CATEGORIES.iter().enumerate().map(
                |(ix, label)| {
                    let count = self.data.as_ref().map(|data| data.groups[ix].len());
                    Button::new(("extension-category", ix))
                        .ghost()
                        .small()
                        .w_full()
                        .justify_start()
                        .selected(self.category == ix)
                        .rounded(cx.theme().radius_tokens().md)
                        .accessibility_label(*label)
                        .when(self.category == ix, |button| {
                            button
                                .bg(cx.theme().secondary)
                                .text_color(cx.theme().secondary_foreground)
                                .font_weight(FontWeight::SEMIBOLD)
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .w_full()
                                .gap_2()
                                .child(Icon::new(CATEGORY_ICONS[ix]).size_4())
                                .child(div().flex_1().min_w_0().truncate().child(*label))
                                .when_some(count, |row, count| {
                                    row.child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(count.to_string()),
                                    )
                                }),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.category = ix;
                            cx.notify();
                        }))
                },
            )))
    }

    pub fn render_content_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .gap_3()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(CATEGORIES[self.category]))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(CATEGORY_DESCRIPTIONS[self.category]),
                    ),
            )
            .child(
                Button::new("refresh-extensions")
                    .ghost()
                    .small()
                    .label(if self.loading { "读取中" } else { "刷新" })
                    .icon(IconName::RefreshCw)
                    .loading(self.loading)
                    .on_click(cx.listener(|this, _, window, cx| this.refresh(window, cx))),
            )
    }

    pub fn scroll_entries(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.data.as_ref().is_none_or(|data| data.groups[self.category].is_empty()) {
            return;
        }
        let key = &event.keystroke;
        if key.modifiers.platform
            || key.modifiers.control
            || key.modifiers.alt
            || key.modifiers.shift
        {
            return;
        }
        let scroll = &self.scroll[self.category];
        let viewport = scroll.viewport_bounds().size.height;
        let maximum = (scroll.content_size().height - viewport).max(Pixels::ZERO);
        let mut offset = scroll.offset();
        let step = window.rem_size() * ENTRY_HEIGHT;
        offset.y = match key.key.as_str() {
            "up" => offset.y + step,
            "down" => offset.y - step,
            "pageup" => offset.y + viewport,
            "pagedown" => offset.y - viewport,
            "home" => Pixels::ZERO,
            "end" => -maximum,
            _ => return,
        }
        .clamp(-maximum, Pixels::ZERO);
        scroll.set_offset(offset);
        self.list_focus.focus(window, cx);
        cx.stop_propagation();
        cx.notify();
    }

    pub fn render_entries(&self, cx: &mut Context<Self>) -> AnyElement {
        let count = self.data.as_ref().map(|data| data.groups[self.category].len());
        if count.is_none_or(|count| count == 0) {
            let (title, description) = if self.loading {
                ("正在读取扩展…".to_owned(), "读取用户与项目的本地定义。")
            } else if self.error.is_some() {
                ("无法读取扩展".to_owned(), "请刷新重试。")
            } else {
                (
                    format!("暂无 {}", CATEGORIES[self.category]),
                    "在用户或项目的 .agents 目录添加定义后刷新。",
                )
            };
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .text_center()
                .gap_2()
                .p_6()
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(Icon::new(CATEGORY_ICONS[self.category]).size_6()),
                )
                .child(div().font_weight(FontWeight::MEDIUM).child(title))
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(description))
                .into_any_element();
        }
        let owner = cx.entity();
        let category = self.category;
        uniform_list(
            ("extension-list", category),
            count.unwrap_or_default(),
            move |range, _, cx| {
                let view = owner.read(cx);
                let Some(data) = &view.data else { return Vec::new() };
                range
                    .filter_map(|ix| data.groups[category].get(ix))
                    .map(|entry| entry.render(owner.downgrade(), view.directory.as_deref(), cx))
                    .collect()
            },
        )
        .track_scroll(&self.scroll[self.category])
        .size_full()
        .into_any_element()
    }
}

impl Render for ExtensionsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(editor) = &self.editor {
            return div().id("extensions-page").size_full().child(editor.clone());
        }
        div()
            .id("extensions-page")
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .text_sm()
            .bg(cx.theme().muted.opacity(0.25))
            .text_color(cx.theme().foreground)
            .child(self.render_header(cx))
            .child(
                div()
                    .flex()
                    .items_stretch()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(self.render_navigation(cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .mr_1p5()
                            .mb_1p5()
                            .flex()
                            .flex_col()
                            .bg(cx.theme().background)
                            .rounded(cx.theme().radius_tokens().lg)
                            .overflow_hidden()
                            .child(self.render_content_header(cx))
                            .when_some(self.error.as_ref(), |content, error| {
                                content.child(
                                    div()
                                        .px_4()
                                        .py_2()
                                        .text_xs()
                                        .text_color(cx.theme().danger)
                                        .child(format!("读取失败：{error} 请刷新重试。")),
                                )
                            })
                            .child(
                                div()
                                    .id("extension-list-viewport")
                                    .relative()
                                    .flex_1()
                                    .min_h_0()
                                    .min_w_0()
                                    .track_focus(&self.list_focus)
                                    .aria_label(format!("{} 列表", CATEGORIES[self.category]))
                                    .aria_description("方向键与 Page Up/Down 滚动，Home/End 跳转首尾，Tab 访问条目操作。")
                                    .border_1()
                                    .border_color(cx.theme().transparent)
                                    .focus_visible(|style| style.border_color(cx.theme().ring))
                                    .on_key_down(cx.listener(Self::scroll_entries))
                                    .child(self.render_entries(cx))
                                    .when(
                                        self.data.as_ref().is_some_and(|data| !data.groups[self.category].is_empty()),
                                        |list| list.vertical_scrollbar(&self.scroll[self.category]),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .px_4()
                                    .py_2()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("仅显示已加载的本地定义；解析失败的文件会被跳过。此页不会执行扩展。"),
                            ),
                    ),
            )
    }
}
