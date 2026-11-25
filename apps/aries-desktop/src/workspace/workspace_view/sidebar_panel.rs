use crate::workspace::workspace_view::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SidebarPanel {
    Sessions,
    Files,
    Git,
}

impl SidebarPanel {
    pub fn sessions() -> Self {
        Self::Sessions
    }

    pub fn files() -> Self {
        Self::Files
    }

    pub fn git() -> Self {
        Self::Git
    }

    fn element_id(self) -> &'static str {
        match self {
            Self::Sessions => "sidebar-sessions",
            Self::Files => "sidebar-files",
            Self::Git => "sidebar-git",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Sessions => "会话",
            Self::Files => "文件",
            Self::Git => "变更",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Sessions => IconName::MessagesSquare,
            Self::Files => IconName::FolderTree,
            Self::Git => IconName::GitBranch,
        }
    }

    pub fn is_available(self, host: &WorkspaceView, cx: &App) -> bool {
        match self {
            Self::Sessions => true,
            Self::Files => host.selected_file_directory(cx).is_some(),
            Self::Git => host.active_view().is_some(),
        }
    }

    pub fn prepare(self, host: &mut WorkspaceView, cx: &mut Context<WorkspaceView>) {
        match self {
            Self::Files => host.sync_file_directory(cx),
            Self::Sessions | Self::Git => {},
        }
    }

    pub fn focus(self, host: &WorkspaceView, window: &mut Window, cx: &mut App) {
        match self {
            Self::Sessions => host.sidebar_sessions_focus_handle.focus(window, cx),
            Self::Files => {
                host.file_tree.update(cx, |tree, cx| tree.focus(window, cx));
            },
            Self::Git => host.sidebar_focus_handle.focus(window, cx),
        }
    }

    pub fn render_tab(
        self,
        host: &WorkspaceView,
        selected: bool,
        cx: &mut Context<WorkspaceView>,
    ) -> AnyElement {
        let button = Button::new(self.element_id())
            .small()
            .ghost()
            .label(self.title())
            .icon(self.icon())
            .selected(selected)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select_sidebar_panel(self, window, cx);
            }));
        let button = match self {
            Self::Sessions => button
                .track_focus(&host.sidebar_sessions_focus_handle)
                .tooltip_with_action(self.tooltip(), &ShowSessionsSidebar::new(), None),
            Self::Files => {
                button.tooltip_with_action(self.tooltip(), &ShowFilesSidebar::new(), None)
            },
            Self::Git => button.tooltip_with_action(self.tooltip(), &ShowGitSidebar::new(), None),
        };
        button.into_any_element()
    }

    pub fn render_panel(
        self,
        host: &WorkspaceView,
        panel_style: StyleRefinement,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) -> AnyElement {
        match self {
            Self::Sessions => host.sidebar.clone().cached(panel_style).into_any_element(),
            Self::Files => Self::render_files(host, panel_style, cx),
            Self::Git => Self::render_git(host, window, cx),
        }
    }

    fn tooltip(self) -> &'static str {
        match self {
            Self::Sessions => "显示会话列表",
            Self::Files => "显示文件树",
            Self::Git => "显示 Git 变更",
        }
    }

    fn render_files(
        host: &WorkspaceView,
        panel_style: StyleRefinement,
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
            .child(host.file_tree.clone().cached(panel_style))
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

    fn render_git(
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
