use agent_client_protocol::Error;
use agent_client_protocol::schema::v1::{
    SessionConfigId, SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOptions, SessionId, SetSessionConfigOptionRequest,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::{Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::acp::Command;
use crate::session::config;
use crate::ui::ChatView;
use crate::ui::palette::Palette;

impl ChatView {
    pub fn render_session_config(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("session-config")
            .flex()
            .flex_col()
            .gap_1()
            .child(div().flex().flex_wrap().items_center().gap_1().when_some(
                self.session_id.as_ref(),
                |row, session_id| {
                    row.children(
                        self.chat
                            .config_options()
                            .iter()
                            .map(|option| self.render_config_option(session_id, option, p, cx)),
                    )
                },
            ))
            .when_some(self.config_error.clone(), |view, error| {
                view.child(div().px_2().text_xs().text_color(p.danger()).child(error))
            })
    }

    fn render_config_option(
        &self,
        session_id: &SessionId,
        option: &SessionConfigOption,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let value = config::value_name(option);
        let label = if matches!(option.kind, SessionConfigKind::Boolean(_)) {
            value.clone()
        } else {
            format!("{}：{value}", option.name)
        };
        let tooltip = match &option.description {
            Some(description) => format!("{label}\n{description}"),
            None => label.clone(),
        };
        let icon = match option.category {
            Some(SessionConfigOptionCategory::Mode) => IconName::Bot,
            Some(SessionConfigOptionCategory::Model) => IconName::Cpu,
            Some(SessionConfigOptionCategory::ThoughtLevel) => IconName::Brain,
            _ => IconName::Settings2,
        };
        let supported = match &option.kind {
            SessionConfigKind::Select(select) => {
                config::select_options(&select.options).next().is_some()
            }
            SessionConfigKind::Boolean(_) => true,
            _ => false,
        };
        let view = cx.weak_entity();
        let session_id = session_id.clone();
        let option = option.clone();
        Button::new(SharedString::from(format!("session-config:{}:{}", session_id, option.id)))
            .ghost()
            .small()
            .max_w(px(260.))
            .min_w_0()
            .text_color(p.muted())
            .disabled(!supported || !self.is_session_idle() || self.disconnected)
            .tooltip(tooltip)
            .accessibility_label(label)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .min_w_0()
                    .text_xs()
                    .child(Icon::new(icon).size(px(13.)).flex_shrink_0())
                    .child(div().min_w_0().truncate().child(value))
                    .child(Icon::new(IconName::ChevronDown).size(px(12.)).flex_shrink_0()),
            )
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                menu = menu
                    .label(option.name.clone())
                    .min_w(px(180.))
                    .max_w(px(360.))
                    .max_h(px(320.))
                    .scrollable(true);
                match &option.kind {
                    SessionConfigKind::Select(select) => {
                        let groups: Vec<_> = match &select.options {
                            SessionConfigSelectOptions::Ungrouped(options) => vec![(None, options)],
                            SessionConfigSelectOptions::Grouped(groups) => groups
                                .iter()
                                .map(|group| (Some(&group.name), &group.options))
                                .collect(),
                            _ => Vec::new(),
                        };
                        for (name, choices) in groups {
                            if let Some(name) = name {
                                menu = menu.separator().label(name.clone());
                            }
                            for choice in choices {
                                menu = menu.item(config_item(
                                    view.clone(),
                                    SetSessionConfigOptionRequest::new(
                                        session_id.clone(),
                                        option.id.clone(),
                                        choice.value.clone(),
                                    ),
                                    choice.name.clone(),
                                    choice.value == select.current_value,
                                ));
                            }
                        }
                    }
                    SessionConfigKind::Boolean(current) => {
                        for (value, name) in [(true, "开启"), (false, "关闭")] {
                            menu = menu.item(config_item(
                                view.clone(),
                                SetSessionConfigOptionRequest::new(
                                    session_id.clone(),
                                    option.id.clone(),
                                    value,
                                ),
                                name.into(),
                                value == current.current_value,
                            ));
                        }
                    }
                    _ => {}
                }
                menu
            })
            .into_any_element()
    }

    fn set_session_config(
        &mut self,
        request: SetSessionConfigOptionRequest,
        cx: &mut Context<Self>,
    ) {
        if self.session_id.as_ref() != Some(&request.session_id)
            || !self.is_session_idle()
            || self.disconnected
            || !self.chat.config_options().iter().any(|option| {
                option.id == request.config_id && config::can_select(option, &request.value)
            })
        {
            return;
        }
        let config_id = request.config_id.clone();
        self.config_error = None;
        if self.bridge.as_ref().is_some_and(|bridge| {
            bridge.send(Command::SetSessionConfigOption(Box::new(request))).is_ok()
        }) {
            self.pending_config = Some(config_id);
        } else {
            self.config_error = Some("无法更新会话配置，请重新连接后重试。".into());
        }
        cx.notify();
    }

    pub fn finish_session_config(
        &mut self,
        session_id: SessionId,
        config_id: SessionConfigId,
        result: Result<Vec<SessionConfigOption>, Error>,
        cx: &mut Context<Self>,
    ) {
        if self.session_id.as_ref() != Some(&session_id)
            || self.pending_config.as_ref() != Some(&config_id)
        {
            return;
        }
        self.pending_config = None;
        match result {
            Ok(options) => {
                self.chat.set_config_options(options);
                self.config_error = None;
            }
            Err(error) => self.config_error = Some(format!("无法更新会话配置：{error}")),
        }
        cx.notify();
    }
}

fn config_item(
    view: WeakEntity<ChatView>,
    request: SetSessionConfigOptionRequest,
    name: String,
    selected: bool,
) -> PopupMenuItem {
    PopupMenuItem::new(name).checked(selected).on_click(move |_, _, cx| {
        let _ = view.update(cx, |this, cx| this.set_session_config(request.clone(), cx));
    })
}
