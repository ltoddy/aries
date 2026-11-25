use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::{Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::NewSession;
use crate::workspace::settings::palette::Palette;
use crate::workspace::workspace_view::{Shell, WorkspacePage, WorkspaceTab, WorkspaceView};

impl WorkspaceView {
    pub fn render_content(&self, shell: &Shell, cx: &mut Context<Self>) -> AnyElement {
        let Some(tab) = self.active_tab() else {
            return self.render_empty_main(shell.palette, cx);
        };
        match tab {
            WorkspaceTab::Page(tab) => {
                let page = self.render_page(tab.page(), shell.style.clone());
                div().size_full().child(page).into_any_element()
            },
            WorkspaceTab::Diff(tab) => {
                if tab.view().read(cx).is_open() {
                    tab.view().clone().cached(shell.style.clone()).into_any_element()
                } else {
                    render_detail(tab.session().clone().cached(shell.style.clone()))
                }
            },
            WorkspaceTab::File(tab) => {
                render_detail(tab.view().clone().cached(shell.style.clone()))
            },
            WorkspaceTab::Session(tab) => {
                render_detail(tab.view().clone().cached(shell.style.clone()))
            },
        }
    }

    pub fn render_empty_main(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let project = self.cwd.file_name().unwrap_or(self.cwd.as_os_str()).to_string_lossy();
        div()
            .size_full()
            .min_h_0()
            .p_8()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .max_w(rems(22.5))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .text_center()
                    .child(Icon::new(IconName::MessageCircle).size_12().text_color(palette.muted()))
                    .child(
                        div()
                            .mt_2()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(palette.foreground())
                            .child(project.into_owned()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(rems(1.25))
                            .text_color(palette.muted())
                            .child("从侧栏选择会话，或在当前项目开始新的会话。"),
                    )
                    .child(
                        Button::new("empty-new-session")
                            .small()
                            .mt_4()
                            .icon(IconName::Plus)
                            .label("新建会话")
                            .disabled(!self.sidebar_view.read(cx).can_start_new_chat())
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(NewSession), cx)
                            }),
                    )
                    .when(cfg!(target_os = "macos"), |empty| {
                        empty.child(
                            div()
                                .mt_2()
                                .text_xs()
                                .text_color(palette.muted())
                                .child("⌘N 新建会话 · ⌘B 切换侧栏"),
                        )
                    }),
            )
            .into_any_element()
    }

    fn render_page(&self, page: WorkspacePage, style: StyleRefinement) -> AnyElement {
        match page {
            WorkspacePage::Settings => self.settings.clone().cached(style).into_any_element(),
            WorkspacePage::Extensions => self.extensions.clone().cached(style).into_any_element(),
            WorkspacePage::Usage => self.usage.clone().cached(style).into_any_element(),
        }
    }
}

fn render_detail(element: impl IntoElement) -> AnyElement {
    div().relative().size_full().min_w_0().min_h_0().child(element).into_any_element()
}
