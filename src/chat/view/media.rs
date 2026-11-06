use std::sync::Arc;

use agent_client_protocol::schema::MaybeUndefined;
use agent_client_protocol::v1::schema::{ContentBlock, SessionUpdate};
use base64::Engine;
use gpui_kit::{Image, ImageFormat};

use crate::chat::model::chat::Role;
use crate::chat::view::ChatView;

pub fn content_image(content: &ContentBlock) -> Option<Arc<Image>> {
    let ContentBlock::Image(image) = content else {
        return None;
    };
    let format = ImageFormat::from_mime_type(&image.mime_type)?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(&image.data).ok()?;
    Some(Arc::new(Image::from_bytes(format, bytes)))
}

fn images(update: &SessionUpdate) -> Vec<Arc<Image>> {
    match update {
        SessionUpdate::UserMessageChunk(chunk)
        | SessionUpdate::AgentMessageChunk(chunk)
        | SessionUpdate::AgentThoughtChunk(chunk) => {
            content_image(&chunk.content).into_iter().collect()
        }
        SessionUpdate::CompactionUpdate(update) => match &update.summary {
            MaybeUndefined::Value(summary) => summary.iter().filter_map(content_image).collect(),
            _ => Vec::new(),
        },
        SessionUpdate::CompactionSummaryChunk(chunk) => {
            content_image(&chunk.content).into_iter().collect()
        }
        _ => Vec::new(),
    }
}

impl ChatView {
    pub fn cache_message_images(&mut self, index: usize) {
        let message = &self.chat.messages()[index];
        let Some(notification) = message.updates().last() else {
            return;
        };
        let update = &notification.update;
        if message.tool_call().is_some() {
            return;
        }
        if let SessionUpdate::UserMessageChunk(chunk) = update {
            if let Some(image) = content_image(&chunk.content) {
                self.attachment_images.insert((index, message.updates().len() - 1), image);
            }
            return;
        }
        if let SessionUpdate::CompactionUpdate(update) = update
            && update.summary.is_undefined()
        {
            return;
        }
        let images = images(update);
        if matches!(update, SessionUpdate::CompactionSummaryChunk(_)) {
            if !images.is_empty() {
                self.images.entry(index).or_default().extend(images);
            }
        } else if images.is_empty() {
            self.images.remove(&index);
        } else {
            self.images.insert(index, images);
        }
    }

    pub fn rebuild_images(&mut self) {
        self.images.clear();
        self.attachment_images.clear();
    }

    pub fn cache_visible_images(&mut self, index: usize) {
        if self.images.contains_key(&index) {
            return;
        }
        let message = &self.chat.messages()[index];
        if message.role() == Role::User || message.tool_call().is_some() {
            return;
        }
        let images = if let Some(compaction) = message.compaction() {
            match &compaction.summary {
                MaybeUndefined::Value(summary) => {
                    summary.iter().filter_map(content_image).collect()
                }
                _ => Vec::new(),
            }
        } else {
            message
                .updates()
                .last()
                .map(|notification| images(&notification.update))
                .unwrap_or_default()
        };
        if !images.is_empty() {
            self.images.insert(index, images);
        }
    }

    pub fn cache_attachment_image(
        &mut self,
        message_index: usize,
        attachment_index: usize,
        content: &ContentBlock,
    ) {
        if self.attachment_images.contains_key(&(message_index, attachment_index)) {
            return;
        }
        if let Some(image) = content_image(content) {
            self.attachment_images.insert((message_index, attachment_index), image);
        }
    }
}
