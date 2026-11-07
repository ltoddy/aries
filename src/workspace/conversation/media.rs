use std::collections::HashMap;
use std::sync::Arc;

use agent_client_protocol::schema::MaybeUndefined;
use agent_client_protocol::v1::schema::{ContentBlock, SessionUpdate};
use base64::Engine;
use gpui_kit::{Image, ImageFormat};

use crate::workspace::conversation::model::{Message, Role};

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

pub struct MessageImages {
    images: HashMap<usize, Vec<Arc<Image>>>,
    attachment_images: HashMap<(usize, usize), Arc<Image>>,
}

impl MessageImages {
    pub fn new() -> Self {
        Self { images: HashMap::new(), attachment_images: HashMap::new() }
    }
    pub fn images(&self, index: usize) -> Option<&[Arc<Image>]> {
        self.images.get(&index).map(Vec::as_slice)
    }
    pub fn attachment(&self, message: usize, attachment: usize) -> Option<Arc<Image>> {
        self.attachment_images.get(&(message, attachment)).cloned()
    }

    pub fn cache_message_images(&mut self, index: usize, message: &Message) {
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

    pub fn cache_visible_images(&mut self, index: usize, message: &Message) {
        if self.images.contains_key(&index) {
            return;
        }
        if message.role() == Role::User {
            for (attachment_index, content) in message.attachments() {
                if !self.attachment_images.contains_key(&(index, attachment_index))
                    && let Some(image) = content_image(content)
                {
                    self.attachment_images.insert((index, attachment_index), image);
                }
            }
            return;
        }
        if message.tool_call().is_some() {
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
}
