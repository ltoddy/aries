use std::sync::atomic::{AtomicU64, Ordering};

use agent_client_protocol::v1::schema::ContentBlock;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::{ActiveTheme, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, FontWeight, div, rems};

use crate::workspace::SessionView;
use crate::workspace::conversation::draft::{Draft, PromptAttachment};
use crate::workspace::conversation::single_line;
use crate::workspace::settings::palette::Palette;

const MAX_QUEUED_PROMPTS: usize = 5;

static NEXT_QUEUE_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_PROMPT_ID: AtomicU64 = AtomicU64::new(1);

pub struct PromptQueue {
    id: u64,
    prompts: Vec<QueuedPrompt>,
    expanded: bool,
    automatic: bool,
}

impl PromptQueue {
    pub fn new() -> Self {
        Self {
            id: NEXT_QUEUE_ID.fetch_add(1, Ordering::Relaxed),
            prompts: Vec::new(),
            expanded: true,
            automatic: true,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.prompts.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.prompts.len() >= MAX_QUEUED_PROMPTS
    }

    pub fn push(&mut self, draft: Draft) -> bool {
        if self.is_full() {
            return false;
        }
        let (text, attachments) = draft.into_parts();
        if text.trim().is_empty() && attachments.is_empty() {
            return false;
        }
        self.prompts.push(QueuedPrompt::new(text, attachments));
        true
    }

    pub fn content(&self) -> Vec<ContentBlock> {
        let mut content = Vec::new();
        for prompt in &self.prompts {
            if !prompt.text.trim().is_empty() {
                content.push(ContentBlock::from(prompt.text.clone()));
            }
            content
                .extend(prompt.attachments.iter().map(|attachment| attachment.content().clone()));
        }
        content
    }

    pub fn pause(&mut self) {
        self.automatic = false;
    }

    pub fn is_automatic(&self) -> bool {
        self.automatic
    }

    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
    }

    pub fn remove(&mut self, id: u64) {
        self.prompts.retain(|prompt| prompt.id != id);
    }

    pub fn render(
        &self,
        session: &SessionView,
        p: Palette,
        cx: &mut Context<SessionView>,
    ) -> AnyElement {
        if self.is_empty() {
            return div().into_any_element();
        }
        let busy = session.execution().conversation().is_busy();
        let automatic = busy && self.is_automatic() && !session.execution().is_stopping();
        div()
            .id(("prompt-queue", self.id))
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(p.border())
            .bg(p.input())
            .text_color(p.foreground())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .child(
                        div().flex_shrink_0().text_sm().font_weight(FontWeight::MEDIUM).child(
                            format!("待发送 · {}/{MAX_QUEUED_PROMPTS} 条", self.prompts.len()),
                        ),
                    )
                    .child(
                        div().flex_1().min_w_0().truncate().text_xs().text_color(p.muted()).child(
                            if automatic {
                                "本次回复正常结束后，将按顺序合并发送。"
                            } else {
                                "队列已保留，当前不会自动发送。"
                            },
                        ),
                    )
                    .child(
                        Button::new(("toggle-prompt-queue", self.id))
                            .ghost()
                            .small()
                            .icon(if self.expanded {
                                IconName::ChevronUp
                            } else {
                                IconName::ChevronDown
                            })
                            .label(if self.expanded { "收起" } else { "展开" })
                            .flex_shrink_0()
                            .accessibility_label(if self.expanded {
                                "收起待发送队列"
                            } else {
                                "展开待发送队列"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_prompt_queue(cx);
                            })),
                    ),
            )
            .when(self.expanded, |queue| {
                queue.child(
                    div()
                        .id(("prompt-queue-list", self.id))
                        .min_h_0()
                        .max_h(rems(12.))
                        .overflow_y_scrollbar()
                        .child(
                            div().px_2().pb_2().flex().flex_col().gap_1().children(
                                self.prompts
                                    .iter()
                                    .enumerate()
                                    .map(|(ix, prompt)| prompt.render(ix + 1, p, cx)),
                            ),
                        ),
                )
            })
            .into_any_element()
    }
}

impl Default for PromptQueue {
    fn default() -> Self {
        Self::new()
    }
}

struct QueuedPrompt {
    id: u64,
    text: String,
    preview: String,
    attachments: Vec<PromptAttachment>,
}

impl QueuedPrompt {
    fn new(text: String, attachments: Vec<PromptAttachment>) -> Self {
        let preview = if text.trim().is_empty() {
            format!("{} 个附件", attachments.len())
        } else {
            single_line(&text)
        };
        Self { id: NEXT_PROMPT_ID.fetch_add(1, Ordering::Relaxed), text, preview, attachments }
    }

    fn render(&self, position: usize, p: Palette, cx: &mut Context<SessionView>) -> AnyElement {
        let id = self.id;
        div()
            .id(("queued-prompt", id))
            .min_w_0()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .w_6()
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(p.muted())
                    .child(position.to_string()),
            )
            .child(div().flex_1().min_w_0().truncate().text_sm().child(self.preview.clone()))
            .when(!self.attachments.is_empty() && !self.text.trim().is_empty(), |row| {
                row.child(
                    div()
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(p.muted())
                        .child(format!("{} 个附件", self.attachments.len())),
                )
            })
            .child(
                Button::new(("remove-queued-prompt", id))
                    .ghost()
                    .small()
                    .flex_shrink_0()
                    .label("移除")
                    .accessibility_label(format!("移除第 {position} 条待发送消息"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.remove_queued_prompt(id, window, cx);
                    })),
            )
            .into_any_element()
    }
}
