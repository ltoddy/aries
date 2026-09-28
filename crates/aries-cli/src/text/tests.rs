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
