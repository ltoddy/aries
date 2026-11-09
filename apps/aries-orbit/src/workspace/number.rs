pub fn compact(value: u64) -> String {
    let (scale, suffix) = match value {
        1_000_000_000.. => (1_000_000_000, "B"),
        1_000_000.. => (1_000_000, "M"),
        1_000.. => (1_000, "K"),
        _ => return value.to_string(),
    };
    let tenths = (u128::from(value) * 10 + scale / 2) / scale;
    if tenths % 10 == 0 {
        format!("{}{suffix}", tenths / 10)
    } else {
        format!("{}.{}{suffix}", tenths / 10, tenths % 10)
    }
}
