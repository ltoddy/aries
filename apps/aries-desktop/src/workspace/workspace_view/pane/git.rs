use gpui_kit::*;

use crate::workspace::workspace_view::{Pane, WorkspaceView};

impl Pane {
    pub fn render_git(
        host: &WorkspaceView,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) -> AnyElement {
        let Some(view) = host.active_view().cloned() else {
            return div().size_full().into_any_element();
        };
        let owner = view.clone();
        div()
            .size_full()
            .min_h_0()
            .min_w_0()
            .child(
                view.update(cx, move |session, cx| session.git_panel().render(&owner, window, cx)),
            )
            .into_any_element()
    }
}
