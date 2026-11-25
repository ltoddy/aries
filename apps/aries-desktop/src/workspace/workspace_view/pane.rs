use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Selectable, Sizable};
use gpui_kit::*;

use crate::workspace::workspace_view::WorkspaceView;
use crate::{ShowFilesSidebar, ShowGitSidebar, ShowSessionsSidebar};

mod files;
mod git;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Sessions,
    Files,
    Git,
}

impl Pane {
    pub fn all() -> [Self; 3] {
        [Self::sessions(), Self::files(), Self::git()]
    }

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

    pub fn render_tab(self, host: &WorkspaceView, cx: &mut Context<WorkspaceView>) -> AnyElement {
        let button = Button::new(self.element_id())
            .small()
            .ghost()
            .label(self.title())
            .icon(self.icon())
            .selected(host.active_sidebar_pane(cx) == self)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select_sidebar_pane(self, window, cx);
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

    pub fn render(
        self,
        host: &WorkspaceView,
        style: StyleRefinement,
        window: &mut Window,
        cx: &mut Context<WorkspaceView>,
    ) -> AnyElement {
        match self {
            Self::Sessions => host.sidebar.clone().cached(style).into_any_element(),
            Self::Files => Self::render_files(host, style, cx),
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
}
