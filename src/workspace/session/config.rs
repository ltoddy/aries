use agent_client_protocol::v1::schema::{
    SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOptions, SessionId, SetSessionConfigOptionRequest,
};
use gpui_kit::assets::IconName;
use gpui_kit::base::SelectableText;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::session::model::config;
use crate::workspace::settings::palette::Palette;

pub fn render_session_config(
    owner: &SessionView,
    p: Palette,
    cx: &mut Context<SessionView>,
) -> impl IntoElement {
    div()
        .id("session-config")
        .flex()
        .flex_col()
        .gap_1()
        .child(div().flex().min_w_0().items_center().gap_1().when_some(
            owner.execution().session_id().as_ref(),
            |row, session_id| {
                row.children(
                    owner
                        .execution()
                        .conversation()
                        .config_options()
                        .iter()
                        .map(|option| render_config_option(owner, session_id, option, p, cx)),
                )
            },
        ))
        .when_some(owner.configuration().error().map(str::to_owned), |view, error| {
            view.child(
                div()
                    .px_2()
                    .text_xs()
                    .text_color(p.danger())
                    .child(SelectableText::new("session-config-error-text", error)),
            )
        })
}

fn render_config_option(
    owner: &SessionView,
    session_id: &SessionId,
    option: &SessionConfigOption,
    p: Palette,
    cx: &mut Context<SessionView>,
) -> AnyElement {
    let value = SharedString::from(config::value_name(option).into_owned());
    let label = if matches!(option.kind, SessionConfigKind::Boolean(_)) {
        value.clone()
    } else {
        SharedString::from(format!("{}：{value}", option.name))
    };
    let tooltip = match &option.description {
        Some(description) => SharedString::from(format!("{label}\n{description}")),
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
        .max_w(rems(16.25))
        .min_w_0()
        .text_color(p.muted())
        .disabled(!supported || !owner.is_session_idle() || owner.execution().is_disconnected())
        .tooltip(tooltip)
        .accessibility_label(label)
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .min_w_0()
                .text_xs()
                .child(Icon::new(icon).size(rems(0.8125)).flex_shrink_0())
                .child(div().min_w_0().truncate().child(value))
                .child(Icon::new(IconName::ChevronDown).size(rems(0.75)).flex_shrink_0()),
        )
        .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, cx| {
            menu = menu
                .label(option.name.clone())
                .min_w(cx.theme().font_size * 11.25)
                .max_w(cx.theme().font_size * 22.5)
                .max_h(cx.theme().font_size * 20.)
                .scrollable(true);
            match &option.kind {
                SessionConfigKind::Select(select) => {
                    let groups: Vec<_> = match &select.options {
                        SessionConfigSelectOptions::Ungrouped(options) => vec![(None, options)],
                        SessionConfigSelectOptions::Grouped(groups) => {
                            groups.iter().map(|group| (Some(&group.name), &group.options)).collect()
                        }
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
                            name,
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

fn config_item(
    view: WeakEntity<SessionView>,
    request: SetSessionConfigOptionRequest,
    name: impl Into<SharedString>,
    selected: bool,
) -> PopupMenuItem {
    PopupMenuItem::new(name).checked(selected).on_click(move |_, _, cx| {
        let _ = view.update(cx, |this, cx| this.set_session_config(request.clone(), cx));
    })
}
