pub fn wrap(text: impl Into<String>, max_width: usize) -> String {
    let text = text.into();

    let mut output = String::with_capacity(text.len());
    let mut line_len = 0;
    for word in text.split_whitespace() {
        if line_len + word.len() > max_width && line_len > 0 {
            output.push('\n');
            line_len = 0;
        }
        if line_len > 0 {
            output.push(' ');
            line_len += 1;
        }
        output.push_str(word);
        line_len += word.len();
    }

    output
}
