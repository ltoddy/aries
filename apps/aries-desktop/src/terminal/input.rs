use std::ops::Range;

use alacritty_terminal::term::TermMode;
use gpui_kit::{
    Bounds, ClipboardItem, Context, EntityInputHandler, Keystroke, Pixels, Point, UTF16Selection,
    Window,
};

use crate::terminal::TerminalView;

pub fn key_bytes(key: &Keystroke, mode: TermMode) -> Option<Vec<u8>> {
    let modifiers = key.modifiers;
    if modifiers.platform {
        return None;
    }
    let modifier = 1
        + u8::from(modifiers.shift)
        + 2 * u8::from(modifiers.alt)
        + 4 * u8::from(modifiers.control);
    let application = mode.contains(TermMode::APP_CURSOR);
    let sequence = match key.key.as_str() {
        "enter" => "\r".to_owned(),
        "escape" => "\x1b".to_owned(),
        "backspace" => if modifiers.control { "\x08" } else { "\x7f" }.to_owned(),
        "tab" if modifiers.shift => "\x1b[Z".to_owned(),
        "tab" => "\t".to_owned(),
        "up" | "down" | "right" | "left" | "home" | "end" => {
            let code = match key.key.as_str() {
                "up" => 'A',
                "down" => 'B',
                "right" => 'C',
                "left" => 'D',
                "home" => 'H',
                _ => 'F',
            };
            if modifier != 1 {
                format!("\x1b[1;{modifier}{code}")
            } else if application {
                format!("\x1bO{code}")
            } else {
                format!("\x1b[{code}")
            }
        },
        "insert" | "delete" | "pageup" | "pagedown" | "f5" | "f6" | "f7" | "f8" | "f9" | "f10"
        | "f11" | "f12" => {
            let code = match key.key.as_str() {
                "insert" => 2,
                "delete" => 3,
                "pageup" => 5,
                "pagedown" => 6,
                "f5" => 15,
                "f6" => 17,
                "f7" => 18,
                "f8" => 19,
                "f9" => 20,
                "f10" => 21,
                "f11" => 23,
                _ => 24,
            };
            if modifier == 1 { format!("\x1b[{code}~") } else { format!("\x1b[{code};{modifier}~") }
        },
        "f1" | "f2" | "f3" | "f4" => {
            let code = match key.key.as_str() {
                "f1" => 'P',
                "f2" => 'Q',
                "f3" => 'R',
                _ => 'S',
            };
            if modifier == 1 { format!("\x1bO{code}") } else { format!("\x1b[1;{modifier}{code}") }
        },
        _ if modifiers.control => {
            let character = if key.key == "space" { ' ' } else { key.key.chars().next()? };
            let byte = match character.to_ascii_lowercase() {
                'a'..='z' => character.to_ascii_lowercase() as u8 - b'a' + 1,
                ' ' | '@' | '2' => 0,
                '[' | '3' => 27,
                '\\' | '4' => 28,
                ']' | '5' => 29,
                '^' | '6' => 30,
                '_' | '7' | '/' => 31,
                '?' | '8' => 127,
                _ => return None,
            };
            return Some(if modifiers.alt { vec![27, byte] } else { vec![byte] });
        },
        _ if modifiers.alt => {
            let text = key.key_char.as_deref().unwrap_or(&key.key);
            if text.chars().count() != 1 {
                return None;
            }
            return Some(format!("\x1b{text}").into_bytes());
        },
        _ => return None,
    };
    let prefix_alt =
        modifiers.alt && matches!(key.key.as_str(), "enter" | "escape" | "backspace" | "tab");
    Some(if prefix_alt { format!("\x1b{sequence}").into_bytes() } else { sequence.into_bytes() })
}

fn byte_offset(text: &str, offset: usize) -> usize {
    let mut utf16 = 0;
    for (byte, character) in text.char_indices() {
        if utf16 >= offset {
            return byte;
        }
        utf16 += character.len_utf16();
    }
    text.len()
}

fn replace_preedit(text: &mut String, range: Option<Range<usize>>, replacement: &str) {
    let range = range.unwrap_or(0..text.encode_utf16().count());
    let start = byte_offset(text, range.start);
    let end = byte_offset(text, range.end.max(range.start));
    text.replace_range(start..end, replacement);
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let start = byte_offset(&self.preedit, range.start);
        let end = byte_offset(&self.preedit, range.end.max(range.start));
        *adjusted = Some(
            self.preedit[..start].encode_utf16().count()
                ..self.preedit[..end].encode_utf16().count(),
        );
        Some(self.preedit[start..end].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection { range: self.preedit_selection.clone(), reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        (!self.preedit.is_empty()).then(|| 0..self.preedit.encode_utf16().count())
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = std::mem::take(&mut self.preedit);
        self.preedit_selection = 0..0;
        self.send(text.into_bytes(), cx);
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let committed = if self.preedit.is_empty() {
            text.to_owned()
        } else {
            replace_preedit(&mut self.preedit, range, text);
            std::mem::take(&mut self.preedit)
        };
        self.preedit_selection = 0..0;
        self.send(committed.into_bytes(), cx);
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selection: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        replace_preedit(&mut self.preedit, range, text);
        let length = self.preedit.encode_utf16().count();
        self.preedit_selection = selection
            .map_or(length..length, |range| range.start.min(length)..range.end.min(length));
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    fn paste(&mut self, item: ClipboardItem, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = item.text() {
            TerminalView::paste(self, &text, cx);
        }
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(self.cursor_bounds)
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.preedit_selection.end)
    }

    fn accepts_text_input(&self, _: &mut Window, _: &mut Context<Self>) -> bool {
        matches!(
            self.status(),
            crate::terminal::Status::Starting | crate::terminal::Status::Running
        )
    }
}
