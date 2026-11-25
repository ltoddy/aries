use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::workspace_view::{Pane, WorkspaceView};

impl Pane {
    pub fn render_files(
        host: &WorkspaceView,
        style: StyleRefinement,
        cx: &mut Context<WorkspaceView>,
    ) -> AnyElement {
        let saving = host.has_saving_file(cx);
        div()
            .size_full()
            .relative()
            .capture_any_mouse_down(cx.listener(|this, _, window, cx| {
                if this.has_saving_file(cx) {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "q"
                    && (event.keystroke.modifiers.platform || event.keystroke.modifiers.control)
                {
                    return;
                }
                if this.has_saving_file(cx) {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }))
            .child(host.file_tree.clone().cached(style))
            .when(saving, |tree| {
                tree.child(
                    div()
                        .absolute()
                        .inset_0()
                        .occlude()
                        .on_any_mouse_down(|_, _, cx| cx.stop_propagation()),
                )
            })
            .into_any_element()
    }
}
