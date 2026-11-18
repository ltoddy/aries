use crate::workspace::conversation::composer::*;
use crate::workspace::settings::palette::Palette;

impl Composer {
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
        self.select_command(&command.name, window, cx);
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
}
