pub const MAX_SCAN_SIZE: usize = 1024;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ContentType {
    Binary,
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Utf32Le,
    Utf32Be,
}

impl ContentType {
    pub fn is_binary(self) -> bool {
        self == ContentType::Binary
    }
}

static BYTE_ORDER_MARKS: &[(&[u8], ContentType)] = &[
    (&[0xEF, 0xBB, 0xBF], ContentType::Utf8Bom),
    (&[0x00, 0x00, 0xFE, 0xFF], ContentType::Utf32Be),
    (&[0xFF, 0xFE, 0x00, 0x00], ContentType::Utf32Le),
    (&[0xFE, 0xFF], ContentType::Utf16Be),
    (&[0xFF, 0xFE], ContentType::Utf16Le),
];

static MAGIC_NUMBERS: [&[u8]; 2] = [b"%PDF", b"\x89PNG"];

pub fn inspect(buffer: &[u8]) -> ContentType {
    for &(mark, content_type) in BYTE_ORDER_MARKS {
        if buffer.starts_with(mark) {
            return content_type;
        }
    }

    let scan_size = buffer.len().min(MAX_SCAN_SIZE);
    if buffer[..scan_size].contains(&0x00u8) {
        return ContentType::Binary;
    }

    for magic in MAGIC_NUMBERS {
        if buffer.starts_with(magic) {
            return ContentType::Binary;
        }
    }

    ContentType::Utf8
}
