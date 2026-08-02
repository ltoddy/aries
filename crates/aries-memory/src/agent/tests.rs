// This file contains tests generated with AI assistance.

use super::*;

const SUFFIX: &str = "\n\n（MEMORY.md 索引过长，已截断。请考虑整理合并记忆。）";

#[test]
fn short_content_is_returned_as_is() {
    let content = "- [a.md](a.md) — desc\n- [b.md](b.md) — desc";
    assert_eq!(truncate_manifest(content), content);
}

#[test]
fn empty_content_is_returned_as_is() {
    assert_eq!(truncate_manifest(""), "");
}

#[test]
fn line_count_at_limit_is_not_truncated() {
    // 恰好 MAX_MANIFEST_LINES 行：count() > MAX 为假，不截断
    let content = (0..MAX_MANIFEST_LINES).map(|i| format!("line{i}")).join("\n");
    let out = truncate_manifest(&content);
    assert_eq!(out, content);
    assert!(!out.ends_with(SUFFIX));
}

#[test]
fn line_count_over_limit_is_truncated() {
    // MAX_MANIFEST_LINES + 1 行：保留前 MAX 行并追加提示
    let content = (0..MAX_MANIFEST_LINES + 1).map(|i| format!("line{i}")).join("\n");
    let out = truncate_manifest(&content);

    let expected_body = (0..MAX_MANIFEST_LINES).map(|i| format!("line{i}")).join("\n");
    assert_eq!(out, format!("{expected_body}{SUFFIX}"));
    // 只保留了前 MAX 行（提示占额外两行）
    assert!(!out.contains(&format!("line{MAX_MANIFEST_LINES}")));
}

#[test]
fn byte_length_over_limit_is_truncated() {
    // 单行超过字节上限：触发字节截断并追加提示
    let content = "a".repeat(MAX_MANIFEST_BYTES + 100);
    let out = truncate_manifest(&content);

    assert!(out.ends_with(SUFFIX));
    let body_len = out.len() - SUFFIX.len();
    assert!(body_len <= MAX_MANIFEST_BYTES);
}

#[test]
fn byte_truncation_respects_char_boundary() {
    // 用 3 字节的多字节字符填充，使字节上限大概率落在字符中间，
    // 验证在字符边界处安全回退、不 panic 且切出的是合法 UTF-8。
    let ch = '好'; // 3 字节
    let content = ch.to_string().repeat(MAX_MANIFEST_BYTES / ch.len_utf8() + 50);
    let out = truncate_manifest(&content);

    assert!(out.ends_with(SUFFIX));
    let body = &out[..out.len() - SUFFIX.len()];
    // 未截断到非字符边界（body 是合法 UTF-8 才能取到这里），且不超过上限
    assert!(body.len() <= MAX_MANIFEST_BYTES);
    assert!(body.chars().all(|c| c == ch));
}
