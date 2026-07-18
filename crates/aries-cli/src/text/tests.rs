// This file contains tests generated with AI assistance.

use super::*;

// --- wrap ---

#[test]
fn wrap_short_text_fits_in_one_line() {
    assert_eq!(wrap("hello world", 20), "hello world");
}

#[test]
fn wrap_long_text_breaks_at_width() {
    let input = "this is a very long description that should wrap";
    let output = wrap(input, 20);
    assert_eq!(output.lines().count(), 3);
}

#[test]
fn wrap_word_longer_than_width_does_not_panic() {
    let input = "supercalifragilisticexpialidocious word";
    let output = wrap(input, 10);
    // The long word sits on its own line, short word on the next.
    assert_eq!(output, "supercalifragilisticexpialidocious\nword");
}

#[test]
fn wrap_empty_string_returns_empty() {
    assert_eq!(wrap("", 50), "");
}

// --- preview ---

#[test]
fn preview_short_content_shows_all_lines() {
    let input = "line1\nline2\nline3";
    let output = preview(input);
    assert_eq!(output, "| line1\n| line2\n| line3");
}

#[test]
fn preview_long_content_truncates_at_five_lines() {
    let input = "a\nb\nc\nd\ne\nf\ng";
    let output = preview(input);
    assert_eq!(output, "| a\n| b\n| c\n| d\n| e\n+ ... (2 more lines truncated)");
}

#[test]
fn preview_exactly_five_lines_shows_all() {
    let input = "1\n2\n3\n4\n5";
    let output = preview(input);
    assert_eq!(output.lines().count(), 5);
    assert!(!output.contains("more lines truncated"));
}

#[test]
fn preview_empty_string_returns_empty() {
    let output = preview("");
    assert_eq!(output, "");
}
