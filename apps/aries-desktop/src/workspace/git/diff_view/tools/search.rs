use crate::workspace::git::diff_view::tools::*;

impl GitDiffView {
    pub fn initialize_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_input.is_some() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("搜索 diff（区分大小写）"));
        self.search_subscription = Some(cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.update_search(cx);
            }
        }));
        self.search_input = Some(input);
    }

    pub fn show_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_open = true;
        if let Some(input) = &self.search_input {
            input.focus_handle(cx).focus(window, cx);
        }
        self.update_search(cx);
        cx.notify();
    }

    pub fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_open = false;
        self.search_revision = self.search_revision.wrapping_add(1);
        self.search_task = None;
        self.search_loading = false;
        self.matches.clear();
        self.match_index = None;
        self.decorations.clear();
        self.state.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    pub fn update_search(&mut self, cx: &mut Context<Self>) {
        self.search_revision = self.search_revision.wrapping_add(1);
        self.search_task = None;
        self.matches.clear();
        self.match_index = None;
        self.decorations.clear();
        self.search_loading = false;
        if !self.search_open {
            return;
        }
        let Some(model) = self.model.clone() else {
            return;
        };
        let query =
            self.search_input.as_ref().map(|input| input.read(cx).value()).unwrap_or_default();
        if query.is_empty() {
            cx.notify();
            return;
        }
        let Some(file) = self.state.read(cx).files().first() else {
            return;
        };
        let path = file.path().clone();
        let revision = self.search_revision;
        self.search_loading = true;
        self.search_task = Some(cx.spawn(async move |view, cx| {
            let matches = cx
                .background_spawn(async move {
                    let mut matches = Vec::new();
                    for line in model.source_lines() {
                        if !line.content().contains(query.as_str()) {
                            continue;
                        }
                        let (side, number) = match (line.original_line(), line.modified_line()) {
                            (_, Some(number)) => (DiffSide::Modified, number),
                            (Some(number), _) => (DiffSide::Original, number),
                            _ => continue,
                        };
                        matches.push(DiffLinePosition::new(path.clone(), side, number as usize));
                    }
                    matches
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.search_revision != revision {
                    return;
                }
                this.search_loading = false;
                this.matches = matches;
                this.match_index = (!this.matches.is_empty()).then_some(0);
                this.update_decorations(cx);
                this.reveal_match(cx);
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub fn update_decorations(&mut self, cx: &App) {
        self.decorations = self
            .matches
            .iter()
            .take(if self.matches.len() <= 128 { self.matches.len() } else { 0 })
            .map(|position| {
                DiffDecoration::new(DiffLineRange::new(
                    position.path().clone(),
                    position.side(),
                    position.line(),
                    position.line(),
                ))
                .with_background(cx.theme().primary.opacity(0.18))
            })
            .collect();
        if let Some(position) = self.match_index.and_then(|index| self.matches.get(index)) {
            self.decorations.push(
                DiffDecoration::new(DiffLineRange::new(
                    position.path().clone(),
                    position.side(),
                    position.line(),
                    position.line(),
                ))
                .with_background(cx.theme().primary.opacity(0.4)),
            );
        }
    }

    pub fn reveal_match(&mut self, cx: &mut Context<Self>) {
        if let Some(position) = self.match_index.and_then(|index| self.matches.get(index)).cloned()
        {
            self.set_location(position.clone(), cx);
            self.state.update(cx, |state, cx| state.scroll_to_line(position, cx));
        }
    }

    pub fn move_match(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let count = self.matches.len();
        if count == 0 {
            return;
        }
        let current = self.match_index.unwrap_or(0);
        self.match_index =
            Some(if backwards { (current + count - 1) % count } else { (current + 1) % count });
        self.update_decorations(cx);
        self.reveal_match(cx);
        cx.notify();
    }

    pub fn render_search(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(input) = &self.search_input else {
            return div().into_any_element();
        };
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .flex_shrink_0()
            .child(div().flex_1().min_w_0().child(Input::new(input).small()))
            .child(
                div()
                    .flex_shrink_0()
                    .text_xs()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.search_loading {
                        "正在搜索".to_owned()
                    } else {
                        format!(
                            "{} / {}",
                            self.match_index.map_or(0, |index| index + 1),
                            self.matches.len()
                        )
                    }),
            )
            .child(
                Button::new("previous-diff-match")
                    .small()
                    .ghost()
                    .compact()
                    .icon(IconName::ChevronUp)
                    .accessibility_label("上一个匹配")
                    .disabled(self.matches.is_empty())
                    .on_click(cx.listener(|this, _, _, cx| this.move_match(true, cx))),
            )
            .child(
                Button::new("next-diff-match")
                    .small()
                    .ghost()
                    .compact()
                    .icon(IconName::ChevronDown)
                    .accessibility_label("下一个匹配")
                    .disabled(self.matches.is_empty())
                    .on_click(cx.listener(|this, _, _, cx| this.move_match(false, cx))),
            )
            .child(
                Button::new("close-diff-search")
                    .small()
                    .ghost()
                    .compact()
                    .icon(IconName::X)
                    .accessibility_label("关闭搜索")
                    .on_click(cx.listener(|this, _, window, cx| this.close_search(window, cx))),
            )
            .into_any_element()
    }
}
