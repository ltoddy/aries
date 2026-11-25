use gpui_kit::component::ActiveTheme;
use gpui_kit::*;

use crate::workspace::settings::palette::Palette;
use crate::workspace::sidebar::Sidebar;
use crate::workspace::workspace_view::{Pane, WorkspaceView};

impl WorkspaceView {
    pub fn select_sidebar_pane(&mut self, pane: Pane, window: &mut Window, cx: &mut Context<Self>) {
        let pane = if pane.is_available(self, cx) { pane } else { Pane::sessions() };
        self.sidebar_pane = pane;
        self.sidebar_visible = true;
        pane.prepare(self, cx);
        let view = cx.weak_entity();
        let revision = self.file_navigation_revision;
        let active_id = self.active_id();
        window.on_next_frame(move |window, cx| {
            let _ = view.update(cx, |this, cx| {
                if this.sidebar_pane != pane
                    || !this.sidebar_visible
                    || this.file_navigation_revision != revision
                    || this.active_id() != active_id
                {
                    return;
                }
                let minimum = cx.theme().font_size * 40.75;
                if window.viewport_size().width < minimum {
                    this.sidebar_toggle_focus_handle.focus(window, cx);
                } else {
                    pane.focus(this, window, cx);
                }
            });
        });
        cx.notify();
    }

    pub fn render_sidebar(
        &self,
        style: StyleRefinement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let pane = self.active_sidebar_pane(cx);
        div()
            .size_full()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .child(self.render_sidebar_tabs(cx))
            .child(div().flex_1().min_h_0().min_w_0().child(pane.render(self, style, window, cx)))
            .child(Sidebar::render_footer(
                Some(cx.weak_entity()),
                Palette::new(cx.theme().mode),
                self.is_settings_visible(),
                self.is_usage_visible(),
                self.is_extensions_visible(),
                cx,
            ))
            .into_any_element()
    }

    fn render_sidebar_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs: Vec<_> =
            Pane::all().into_iter().filter(|pane| pane.is_available(self, cx)).collect();
        div()
            .flex()
            .items_center()
            .gap_1()
            .px_2()
            .py_1()
            .children(tabs.into_iter().map(|pane| pane.render_tab(self, cx)))
    }

    pub fn active_sidebar_pane(&self, cx: &App) -> Pane {
        if self.sidebar_pane.is_available(self, cx) { self.sidebar_pane } else { Pane::sessions() }
    }

    pub fn toggle_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        if !self.sidebar_visible && self.sidebar_focus_handle.contains_focused(window, cx) {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }
}
