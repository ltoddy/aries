use crate::workspace::session::session_view::*;

impl SessionView {
    pub fn render_session_delete_popup(
        &self,
        anchor: impl AsRef<str>,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        crate::workspace::session::deletion::render_session_delete_popup(self, anchor, palette, cx)
    }
    pub fn render_session_config(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        crate::workspace::session::config::render_session_config(self, palette, cx)
    }
    pub fn render_execution_context(
        &self,
        palette: Palette,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        crate::workspace::session::execution_context::render_execution_context(self, palette, cx)
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
        palette: Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.sidebar.render(self, self.tab_host.clone(), palette, window, cx)
    }
    fn render_main_area(
        &mut self,
        palette: Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let timeline = self.timeline.render_timeline(
            self.execution.conversation(),
            self.execution.is_loading(),
            palette,
            cx,
        );
        let composer = self.composer.render(self, palette, window, cx);
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .size_full()
            .bg(palette.background())
            .text_color(palette.foreground())
            .flex()
            .flex_col()
            .child(timeline)
            .child(composer)
    }
}

impl Render for SessionView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::new(cx.theme().mode);
        match self.render_mode {
            SessionRenderMode::Sidebar => {
                self.render_sidebar(palette, window, cx).into_any_element()
            },
            SessionRenderMode::Main => {
                self.render_main_area(palette, window, cx).into_any_element()
            },
        }
    }
}
