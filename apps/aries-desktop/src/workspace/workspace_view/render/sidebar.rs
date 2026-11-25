use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::sidebar::Sidebar;
use crate::workspace::workspace_view::{Shell, WorkspaceView};

impl WorkspaceView {
    pub fn render_sidebar(
        &self,
        shell: &Shell,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let panel = self.active_sidebar_panel(cx);
        let mut buttons = Vec::new();
        for candidate in &self.sidebar.panels {
            if candidate.is_available(self, cx) {
                buttons.push(candidate.render_tab(self, *candidate == panel, cx));
            }
        }
        div()
            .size_full()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .child(div().flex().items_center().gap_1().px_2().py_1().children(buttons))
            .child(div().flex_1().min_h_0().min_w_0().child(panel.render_panel(
                self,
                shell.style.clone(),
                window,
                cx,
            )))
            .child(Sidebar::render_footer(
                Some(cx.weak_entity()),
                shell.palette,
                self.active_page(),
                cx,
            ))
            .into_any_element()
    }
}
