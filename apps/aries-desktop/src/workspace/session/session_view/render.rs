use crate::workspace::session::session_view::*;

impl SessionView {
    pub fn render_session_actions(&self, p: Palette) -> impl IntoElement {
        crate::workspace::session::deletion::render_session_actions(self, p)
    }
    pub fn render_session_delete_popup(
        &self,
        anchor: impl AsRef<str>,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        crate::workspace::session::deletion::render_session_delete_popup(self, anchor, p, cx)
    }
    pub fn render_session_config(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        crate::workspace::session::config::render_session_config(self, p, cx)
    }
    pub fn render_execution_context(&self, p: Palette, cx: &Context<Self>) -> impl IntoElement {
        crate::workspace::session::execution_context::render_execution_context(self, p, cx)
    }
    pub fn execution_state(&self) -> ExecutionState {
        [
            (self.execution.is_disconnected(), ExecutionState::Disconnected),
            (self.execution.is_stopping(), ExecutionState::Stopping),
            (self.deletion.is_busy(), ExecutionState::Deleting),
            (self.management.is_pending(), ExecutionState::updating()),
            (self.execution.permission().is_some(), ExecutionState::Permission),
            (self.sidebar.list().is_connecting(), ExecutionState::Connecting),
            (self.execution.is_choosing_directory(), ExecutionState::ChoosingProject),
            (self.execution.loading_id().is_some(), ExecutionState::Loading),
            (self.execution.is_creating(), ExecutionState::Creating),
            (self.configuration.is_pending(), ExecutionState::Configuring),
            (self.execution.conversation().is_busy(), ExecutionState::Running),
            (self.execution.conversation().error().is_some(), ExecutionState::Failed),
        ]
        .into_iter()
        .find_map(|(active, state)| active.then_some(state))
        .unwrap_or(ExecutionState::Idle)
    }
    fn render_sidebar(
        &self,
        p: Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.sidebar.render(self, self.tab_host.clone(), p, window, cx)
    }
    fn render_main_area(
        &mut self,
        p: Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let timeline = self.timeline.render_timeline(
            self.execution.conversation(),
            self.execution.is_loading(),
            p,
            cx,
        );
        let composer = self.composer.render(self, p, window, cx);
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .size_full()
            .bg(p.background())
            .text_color(p.foreground())
            .flex()
            .flex_col()
            .child(timeline)
            .child(composer)
    }
    fn render_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx.theme().mode);
        div()
            .size_full()
            .flex()
            .bg(p.background())
            .text_color(p.foreground())
            .font_family(cx.theme().font_family.clone())
            .text_sm()
            .child(
                h_resizable("layout")
                    .child(
                        resizable_panel()
                            .size(cx.theme().font_size * 18.)
                            .size_range(cx.theme().font_size * 12.5..cx.theme().font_size * 32.5)
                            .child(self.render_sidebar(p, window, cx)),
                    )
                    .child(resizable_panel().child(self.render_main_area(p, window, cx))),
            )
    }
}

impl Render for SessionView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sidebar.cleanup(cx);
        let p = Palette::new(cx.theme().mode);
        match self.render_mode {
            SessionRenderMode::Full => self.render_layout(window, cx).into_any_element(),
            SessionRenderMode::Sidebar => self.render_sidebar(p, window, cx).into_any_element(),
            SessionRenderMode::Main => self.render_main_area(p, window, cx).into_any_element(),
        }
    }
}
