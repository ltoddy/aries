use std::future::Future;
use std::path::PathBuf;

use agent_client_protocol::v1::schema::{AvailableCommand, ContentBlock, PromptCapabilities};
use gpui_kit::assets::IconName;
use gpui_kit::base::motion::{Presence, Transition};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{
    Enter, Escape, IndentInline, InputEvent, MoveDown, MoveUp, Textarea, TextareaState,
};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable, Size, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;
use thiserror::Error;

use crate::error::report;
use crate::workspace::SessionView;
use crate::workspace::conversation::draft::{
    AttachmentError, Draft, MAX_ATTACHMENT_BYTES, MAX_ATTACHMENTS, PromptAttachment,
};
use crate::workspace::conversation::slash_commands::{CommandMenu, CommandMenuAction, SlashMenu};

mod attachments;
mod commands;
mod render;

pub struct Composer {
    input: Entity<TextareaState>,
    command_menu: SlashMenu,
    attachments: Vec<PromptAttachment>,
    attachment_task: Option<Task<()>>,
    attachment_revision: u64,
}

impl Composer {
    pub fn new(window: &mut Window, cx: &mut Context<SessionView>) -> Self {
        let input =
            cx.new(|cx| TextareaState::new(window, cx).placeholder("发送消息…").auto_grow(2, 6));
        Self {
            input,
            command_menu: SlashMenu::default(),
            attachments: Vec::new(),
            attachment_task: None,
            attachment_revision: 0,
        }
    }
    pub fn input(&self) -> &Entity<TextareaState> {
        &self.input
    }
    pub fn is_loading(&self) -> bool {
        self.attachment_task.is_some()
    }
    pub fn has_content(&self, cx: &App) -> bool {
        self.attachment_task.is_none()
            && (!self.input.read(cx).value().trim().is_empty() || !self.attachments.is_empty())
    }

    pub fn content(&self, cx: &App) -> Vec<ContentBlock> {
        let text = self.input.read(cx).value();
        let mut content = Vec::new();
        if !text.trim().is_empty() {
            content.push(ContentBlock::from(text.trim()));
        }
        content.extend(self.attachments.iter().map(|attachment| attachment.content().clone()));
        content
    }

    pub fn draft(&self, cx: &App) -> Draft {
        Draft::new(self.input.read(cx).value().to_string(), self.attachments.clone())
    }

    pub fn restore_draft(
        &mut self,
        draft: Draft,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        let (text, attachments) = draft.into_parts();
        self.clear_attachments();
        self.attachments = attachments;
        self.input.update(cx, |input, cx| input.set_value(text, window, cx));
    }

    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<SessionView>) {
        self.clear_attachments();
        self.reset_menu();
        self.input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
    }

    pub fn clear_attachments(&mut self) {
        self.attachment_revision = self.attachment_revision.wrapping_add(1);
        self.attachment_task = None;
        self.attachments.clear();
    }
}
