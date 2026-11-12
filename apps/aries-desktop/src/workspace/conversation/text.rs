pub fn single_line(text: impl AsRef<str>) -> String {
    text.as_ref().split_whitespace().collect::<Vec<_>>().join(" ")
}
