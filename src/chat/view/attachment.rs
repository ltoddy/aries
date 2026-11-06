use agent_client_protocol::v1::schema::{ContentBlock, EmbeddedResourceResource};
use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::view::ChatView;
use crate::chat::view::palette::Palette;

#[derive(Debug)]
struct AttachmentInfo<'a> {
    name: &'a str,
    kind: &'static str,
    icon: IconName,
    mime_type: Option<&'a str>,
    uri: Option<&'a str>,
    description: Option<&'a str>,
}

impl<'a> From<&'a ContentBlock> for AttachmentInfo<'a> {
    fn from(content: &'a ContentBlock) -> Self {
        match content {
            ContentBlock::Image(image) => Self {
                name: file_name(image.uri.as_deref().unwrap_or_default(), "图片"),
                kind: "图片",
                icon: IconName::Image,
                mime_type: Some(&image.mime_type),
                uri: image.uri.as_deref(),
                description: None,
            },
            ContentBlock::Audio(audio) => Self {
                name: "音频",
                kind: "音频",
                icon: IconName::FileMusic,
                mime_type: Some(&audio.mime_type),
                uri: None,
                description: None,
            },
            ContentBlock::ResourceLink(resource) => Self {
                name: resource
                    .title
                    .as_deref()
                    .filter(|title| !title.trim().is_empty())
                    .or_else(|| {
                        (!resource.name.trim().is_empty()).then_some(resource.name.as_str())
                    })
                    .unwrap_or_else(|| file_name(&resource.uri, "资源链接")),
                kind: "资源链接",
                icon: IconName::Link,
                mime_type: resource.mime_type.as_deref(),
                uri: Some(&resource.uri),
                description: resource.description.as_deref(),
            },
            ContentBlock::Resource(resource) => {
                let (uri, mime_type, icon) = match &resource.resource {
                    EmbeddedResourceResource::TextResourceContents(resource) => {
                        (resource.uri.as_str(), resource.mime_type.as_deref(), IconName::FileText)
                    }
                    EmbeddedResourceResource::BlobResourceContents(resource) => {
                        (resource.uri.as_str(), resource.mime_type.as_deref(), IconName::File)
                    }
                    _ => ("", None, IconName::File),
                };
                Self {
                    name: file_name(uri, "资源"),
                    kind: "资源",
                    icon,
                    mime_type,
                    uri: Some(uri),
                    description: None,
                }
            }
            _ => Self {
                name: "附件",
                kind: "附件",
                icon: IconName::Paperclip,
                mime_type: None,
                uri: None,
                description: None,
            },
        }
    }
}

fn file_name<'a>(uri: &'a str, fallback: &'a str) -> &'a str {
    uri.split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim_end_matches('/')
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(fallback)
}

impl ChatView {
    pub fn render_attachment(
        &self,
        message_index: usize,
        attachment_index: usize,
        content: &ContentBlock,
        p: Palette,
    ) -> AnyElement {
        let info = AttachmentInfo::from(content);
        let subtitle = info
            .mime_type
            .filter(|mime_type| !mime_type.is_empty())
            .map(|mime_type| format!("{} · {mime_type}", info.kind))
            .unwrap_or_else(|| info.kind.into());
        let details = [Some(info.name), Some(subtitle.as_str()), info.uri, info.description]
            .into_iter()
            .flatten()
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        let details = SharedString::from(details);
        let icon = info.icon;
        let preview = match self.attachment_images.get(&(message_index, attachment_index)) {
            Some(image) => img(image.clone())
                .size_10()
                .rounded(px(6.))
                .object_fit(ObjectFit::Contain)
                .with_fallback(move || Icon::new(icon).size(px(24.)).into_any_element())
                .into_any_element(),
            None => Icon::new(icon).size(px(24.)).into_any_element(),
        };
        div()
            .id(SharedString::from(format!("attachment:{message_index}:{attachment_index}")))
            .aria_label(format!("{}附件：{}", info.kind, info.name))
            .w(px(240.))
            .max_w_full()
            .min_w_0()
            .px_2()
            .py_1()
            .rounded(px(8.))
            .border_1()
            .border_color(p.border())
            .bg(p.input())
            .flex()
            .items_center()
            .gap_2()
            .tooltip(move |window, cx| {
                let details = details.clone();
                Tooltip::element(move |_, _| {
                    div().max_w(px(480.)).whitespace_normal().child(details.clone())
                })
                .build(window, cx)
            })
            .child(
                div()
                    .size_10()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(p.muted())
                    .child(preview),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(info.name.to_owned()),
                    )
                    .child(div().mt_1().truncate().text_xs().text_color(p.muted()).child(subtitle)),
            )
            .into_any_element()
    }
}
