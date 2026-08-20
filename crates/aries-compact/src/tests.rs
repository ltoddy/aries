use rig::message::{Message, ToolResultContent};

use crate::TokenEstimator;

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
fn tool_result_json_estimates_from_serialized_value() {
    let content = ToolResultContent::Json {
        value: serde_json::json!({ "ok": true }),
    };

    assert_eq!(content.estimate_tokens(), "{\"ok\":true}".estimate_tokens());
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
        ToolResultContent::Json {
            value: serde_json::json!([1, 2, 3]),
        },
    ];

    assert_eq!(
        content.estimate_tokens(),
        "abcd".estimate_tokens() + "[1,2,3]".estimate_tokens()
    );
}
