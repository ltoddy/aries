use agent_client_protocol::v1::schema::AvailableCommand;
use gpui_kit::assets::IconName;
use gpui_kit::base::motion::{Presence, Transition};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{
    Enter, Escape, IndentInline, InputEvent, MoveDown, MoveUp, Textarea, TextareaState,
};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::conversation::slash_commands::{CommandMenu, CommandMenuAction, SlashMenu};
use crate::workspace::settings::SendKey;
use crate::workspace::settings::palette::Palette;

pub struct Composer {
    input: Entity<TextareaState>,
    command_menu: SlashMenu,
}

impl Composer {
    pub fn new(window: &mut Window, cx: &mut Context<SessionView>) -> Self {
        let input =
            cx.new(|cx| TextareaState::new(window, cx).placeholder("发送消息…").auto_grow(2, 6));
        Self { input, command_menu: SlashMenu::default() }
    }
    pub fn input(&self) -> &Entity<TextareaState> {
        &self.input
    }
    pub fn reset_menu(&mut self) {
        self.command_menu.reset();
    }
    pub fn input_event(&mut self, event: &InputEvent) {
        match event {
            InputEvent::Change => self.command_menu.reset(),
            InputEvent::Focus => self.command_menu.focus(),
            InputEvent::Blur => self.command_menu.blur(),
            _ => {},
        }
    }

    fn command_candidates<'a>(
        &self,
        commands: &'a [AvailableCommand],
        available: bool,
        cx: &App,
    ) -> Option<Vec<&'a AvailableCommand>> {
        if !self.command_menu.is_open() || !available {
            return None;
        }
        let input = self.input.read(cx);
        let text = input.value();
        let query = text.strip_prefix('/')?;
        if query.chars().any(|character| character.is_whitespace() || character == '/')
            || !input.selected_range().is_empty()
            || input.cursor() != text.len()
        {
            return None;
        }
        let query = query.to_lowercase();
        Some(
            commands
                .iter()
                .filter(|command| command.name.to_lowercase().starts_with(&query))
                .collect(),
        )
    }

    pub fn complete_selected_command(
        &mut self,
        commands: &[AvailableCommand],
        available: bool,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) -> bool {
        let Some(candidates) = self.command_candidates(commands, available, cx) else {
            return false;
        };
        let Some(command) = candidates.get(self.command_menu.selected()) else {
            return false;
        };
        let name = command.name.clone();
        self.select_command(&name, window, cx);
        true
    }

    pub fn select_command(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        let text = format!("/{name} ");
        let end = text.len();
        self.input.update(cx, |input, cx| {
            input.replace_all(text, window, cx);
            input.set_selected_range(end..end, cx);
            input.focus(window, cx);
        });
        self.command_menu.dismiss();
        cx.notify();
    }

    pub fn command_menu_action(
        &mut self,
        action: CommandMenuAction,
        commands: &[AvailableCommand],
        available: bool,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        let Some(count) =
            self.command_candidates(commands, available, cx).map(|candidates| candidates.len())
        else {
            return;
        };
        if self.input.update(cx, |input, cx| input.marked_text_range(window, cx).is_some()) {
            return;
        }
        match action {
            CommandMenuAction::Previous if count > 0 => {
                self.command_menu.move_selection(action, count);
            },
            CommandMenuAction::Next if count > 0 => {
                self.command_menu.move_selection(action, count);
            },
            CommandMenuAction::Complete if count > 0 => {
                self.complete_selected_command(commands, available, window, cx);
            },
            CommandMenuAction::Dismiss => self.command_menu.dismiss(),
            _ => return,
        }
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }

    pub fn render_commands(
        &self,
        commands: &[AvailableCommand],
        available: bool,
        has_session: bool,
        p: Palette,
        cx: &mut Context<SessionView>,
    ) -> Option<AnyElement> {
        let candidates = self.command_candidates(commands, available, cx)?;
        let empty = if !has_session {
            "新建会话后可获取可用命令"
        } else if commands.is_empty() {
            "当前会话暂无可用命令"
        } else {
            "没有匹配的命令"
        };
        let view = cx.weak_entity();
        Some(
            CommandMenu::from_commands(
                &self.command_menu,
                candidates,
                empty,
                p,
                move |name, window, cx| {
                    let _ = view.update(cx, |this, cx| this.select_command(name, window, cx));
                },
            )
            .into_any_element(),
        )
    }
    pub fn render(
        &self,
        session: &SessionView,
        send_key: SendKey,
        commands_available: bool,
        p: Palette,
        cx: &mut Context<SessionView>,
    ) -> impl IntoElement {
        let can_send = session.can_send(cx);
        div().flex_shrink_0().pl_8().pr_4().py_2().bg(p.background()).child(
            div()
                .w_full()
                .max_w(rems(55.))
                .mx_auto()
                .flex()
                .flex_col()
                .gap_2()
                .when_some(session.execution().permission(), |composer, permission| {
                    composer.child(PermissionPrompt {
                        id: ("permission-motion", cx.entity_id()).into(),
                        child: div()
                            .p_3()
                            .rounded(cx.theme().radius_lg)
                            .border_1()
                            .border_color(p.border())
                            .bg(p.input())
                            .child(
                                div()
                                    .mb_2()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(permission.title().to_owned()),
                            )
                            .child(div().flex().flex_wrap().gap_2().children(
                                permission.options().iter().enumerate().map(|(index, option)| {
                                    let id = option.option_id.clone();
                                    Button::new(("permission", index))
                                        .label(option.name.clone())
                                        .small()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.choose_permission(id.clone(), cx)
                                        }))
                                }),
                            ))
                            .into_any_element(),
                    })
                })
                .child(session.render_execution_context(p, cx))
                .child(
                    div()
                        .relative()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .capture_action(cx.listener(|this, _: &MoveUp, window, cx| {
                            this.command_menu_action(CommandMenuAction::Previous, window, cx)
                        }))
                        .capture_action(cx.listener(|this, _: &MoveDown, window, cx| {
                            this.command_menu_action(CommandMenuAction::Next, window, cx)
                        }))
                        .capture_action(cx.listener(|this, action: &Enter, window, cx| {
                            if action.secondary
                                || !this.input().read(cx).focus_handle(cx).is_focused(window)
                                || this.input().update(cx, |input, cx| {
                                    input.marked_text_range(window, cx).is_some()
                                })
                            {
                                return;
                            }
                            if !action.shift {
                                this.command_menu_action(CommandMenuAction::Complete, window, cx);
                                if window.default_prevented() {
                                    return;
                                }
                            }
                            if action.shift == (this.send_key(cx) == SendKey::ShiftEnter) {
                                window.prevent_default();
                                cx.stop_propagation();
                                this.send(window, cx);
                            }
                        }))
                        .capture_action(cx.listener(|this, _: &IndentInline, window, cx| {
                            this.command_menu_action(CommandMenuAction::Complete, window, cx)
                        }))
                        .capture_action(cx.listener(|this, _: &Escape, window, cx| {
                            this.command_menu_action(CommandMenuAction::Dismiss, window, cx)
                        }))
                        .children(self.render_commands(
                            session.execution().conversation().available_commands(),
                            commands_available,
                            session.execution().session_id().is_some(),
                            p,
                            cx,
                        ))
                        .child(ComposerInput {
                            input: self.input.clone(),
                            busy: session.execution().conversation().is_busy(),
                            stopping: session.execution().is_stopping(),
                            can_send,
                            palette: p,
                            on_submit: Box::new(cx.listener(|this, _, window, cx| {
                                if this.execution().conversation().is_busy() {
                                    this.stop(window, cx);
                                } else {
                                    this.send(window, cx);
                                }
                            })),
                        }),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .min_h_6()
                        .child(div().flex_1().min_w_0().when(
                            !session.execution().conversation().config_options().is_empty()
                                || session.configuration().error().is_some(),
                            |footer| footer.child(session.render_session_config(p, cx)),
                        ))
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_xs()
                                .text_color(p.muted())
                                .text_right()
                                .child(format!(
                                    "{} 发送 · {} 换行",
                                    send_key.send_label(),
                                    send_key.newline_label(),
                                )),
                        ),
                ),
        )
    }
}

#[derive(IntoElement)]
struct PermissionPrompt {
    id: ElementId,
    child: AnyElement,
}

impl RenderOnce for PermissionPrompt {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let motion = cx.theme().motion_tokens();
        let distance = motion.distance_short;
        let transition =
            Transition::new(motion.duration_normal).easing(motion.easing_enter.clone());
        let presence = Presence::new(self.id, true).transition(transition).sample(window, cx);
        div().w_full().child(
            div()
                .relative()
                .top(distance * (1. - presence.progress))
                .opacity(presence.progress)
                .child(self.child),
        )
    }
}

#[derive(IntoElement)]
struct ComposerInput {
    input: Entity<TextareaState>,
    busy: bool,
    stopping: bool,
    can_send: bool,
    palette: Palette,
    on_submit: crate::workspace::conversation::ClickHandler,
}
impl RenderOnce for ComposerInput {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self { input, busy, stopping, can_send, palette: p, on_submit } = self;
        div()
            .flex()
            .items_end()
            .gap_2()
            .p_1()
            .rounded(rems(0.875))
            .border_1()
            .border_color(p.border())
            .bg(p.input())
            .child(div().id("composer-input").flex_1().min_w_0().px_1().child(
                Textarea::new(&input).appearance(false).bordered(false).aria_label("消息").w_full(),
            ))
            .child(
                Button::new("send")
                    .primary()
                    .icon(
                        Icon::new(if busy { IconName::Square } else { IconName::Send })
                            .size(rems(1.0625)),
                    )
                    .accessibility_label(if busy { "停止回复" } else { "发送消息" })
                    .size_10()
                    .rounded(cx.theme().radius_lg)
                    .disabled(if busy { stopping } else { !can_send })
                    .on_click(on_submit),
            )
    }
}
