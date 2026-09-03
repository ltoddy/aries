use aries_context::ChatContext;
use rig::message::{
    AssistantContent, Document, DocumentSourceKind, Message, Reasoning, ToolResultContent,
    UserContent,
};

use crate::{TokenEstimator, micro_compact};

#[test]
fn empty_text_estimates_to_zero_tokens() {
    assert_eq!("".estimate_tokens(), 0);
    assert_eq!(String::new().estimate_tokens(), 0);
}

#[test]
fn ascii_text_uses_quarter_token_per_char() {
    assert_eq!("a".estimate_tokens(), 1);
    assert_eq!("abcd".estimate_tokens(), 1);
    assert_eq!("abcde".estimate_tokens(), 2);
}

#[test]
fn cjk_text_uses_higher_token_estimate() {
    assert_eq!("你".estimate_tokens(), 2);
    assert_eq!("你好".estimate_tokens(), 3);
}

#[test]
fn non_cjk_non_ascii_text_uses_half_token_per_char() {
    assert_eq!("é".estimate_tokens(), 1);
    assert_eq!("éé".estimate_tokens(), 1);
    assert_eq!("ééé".estimate_tokens(), 2);
}

#[test]
fn mixed_text_sums_per_character_estimates() {
    assert_eq!("a你é".estimate_tokens(), 2);
}

#[test]
fn encrypted_reasoning_does_not_count_as_prompt_tokens() {
    let content = AssistantContent::Reasoning(Reasoning::encrypted("encrypted payload"));

    assert_eq!(content.estimate_tokens(), 5);
}

#[test]
fn tool_result_json_estimates_from_serialized_value() {
    let content = ToolResultContent::Json { value: serde_json::json!({ "ok": true }) };

    assert_eq!(content.estimate_tokens(), "{\"ok\":true}".estimate_tokens());
}

#[test]
fn string_document_estimates_from_actual_content() {
    let content = UserContent::Document(Document {
        data: DocumentSourceKind::String("opened file metadata".to_owned()),
        media_type: None,
        additional_params: None,
    });

    assert_eq!(content.estimate_tokens(), "opened file metadata".estimate_tokens());
}

#[test]
fn message_slice_applies_conservative_multiplier() {
    let messages = vec![Message::user("abcd"), Message::user("efgh")];

    assert_eq!(messages[0].estimate_tokens(), 1);
    assert_eq!(messages[1].estimate_tokens(), 1);
    assert_eq!(messages.as_slice().estimate_tokens(), 3);
    assert_eq!(messages.estimate_tokens(), 3);
}

#[test]
fn tool_result_list_estimates_sum_nested_contents() {
    let content = vec![
        ToolResultContent::text("abcd"),
        ToolResultContent::Json { value: serde_json::json!([1, 2, 3]) },
    ];

    assert_eq!(content.estimate_tokens(), "abcd".estimate_tokens() + "[1,2,3]".estimate_tokens());
}

#[test]
fn micro_compact_reports_changes_and_replaces_old_tool_results_with_placeholder() {
    let mut messages = tool_result_messages(3);

    assert!(micro_compact(&mut messages, 1));

    assert_old_tool_results_cleared(&messages);
}

#[tokio::test]
async fn overwritten_chat_context_reloads_micro_compacted_placeholders() {
    let tmp = tempfile::TempDir::new().unwrap();
    let context = ChatContext::new(tmp.path()).await.unwrap();
    let messages = tool_result_messages(3);
    context.append(&messages).await;

    let mut compacted = context.history().await.clone();
    assert!(micro_compact(&mut compacted, 1));
    context.overwrite(compacted).await;

    let reloaded = ChatContext::new(tmp.path()).await.unwrap();
    let history = reloaded.history().await;
    assert_old_tool_results_cleared(&history);
}

fn tool_result_messages(count: usize) -> Vec<Message> {
    (0..count)
        .map(|i| Message::User {
            content: vec![UserContent::tool_result(
                format!("call-{i}"),
                "Read",
                vec![ToolResultContent::text(format!("full result {i}"))],
            )],
        })
        .collect()
}

fn assert_old_tool_results_cleared(messages: &[Message]) {
    let serialized = serde_json::to_string(messages).unwrap();
    assert!(serialized.contains("[Old tool result content cleared]"));
    assert!(!serialized.contains("full result 0"));
    assert!(!serialized.contains("full result 1"));
    assert!(serialized.contains("full result 2"));
}
