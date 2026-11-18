use std::fs::{self, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::str::Utf8Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use gpui_kit::*;
use thiserror::Error;

use crate::workspace::files::file_view::{MAX_FILE_BYTES, UTF8_BOM};

static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Error)]
pub enum ReadFileError {
    #[error("无法读取文件：{0}")]
    Io(#[source] io::Error),
    #[error("仅支持普通文件，不能查看目录、FIFO、设备或套接字")]
    NotRegular,
    #[error("文件超过 2 MiB 的查看限制")]
    TooLarge,
    #[error("文件包含二进制控制字符，不能作为文本查看")]
    Binary,
    #[error("文件不是有效的 UTF-8 文本：{0}")]
    InvalidUtf8(#[source] Utf8Error),
}

impl ReadFileError {
    fn io(error: io::Error) -> Self {
        Self::Io(error)
    }

    fn not_regular() -> Self {
        Self::NotRegular
    }

    fn too_large() -> Self {
        Self::TooLarge
    }

    fn binary() -> Self {
        Self::Binary
    }

    fn invalid_utf8(error: Utf8Error) -> Self {
        Self::InvalidUtf8(error)
    }
}

fn open_regular_file(path: &Path) -> Result<fs::File, ReadFileError> {
    let metadata = fs::metadata(path).map_err(ReadFileError::io)?;
    if !metadata.is_file() {
        return Err(ReadFileError::not_regular());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY);
    }
    let file = options.open(path).map_err(ReadFileError::io)?;
    if !file.metadata().map_err(ReadFileError::io)?.is_file() {
        return Err(ReadFileError::not_regular());
    }
    Ok(file)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineEnding {
    Lf,
    CrLf,
    Cr,
}

impl LineEnding {
    fn lf() -> Self {
        Self::Lf
    }

    fn crlf() -> Self {
        Self::CrLf
    }

    fn cr() -> Self {
        Self::Cr
    }

    fn text(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::CrLf => "CRLF",
            Self::Cr => "CR",
        }
    }
}

#[derive(Clone, Debug)]
pub struct FileDocument {
    metadata: fs::Metadata,
    original: Arc<[u8]>,
    pub content: SharedString,
    pub bom: bool,
    endings: Vec<LineEnding>,
    line_ending: LineEnding,
}

impl FileDocument {
    fn new(
        _source_path: PathBuf,
        location: PathBuf,
        original: Vec<u8>,
    ) -> Result<Self, ReadFileError> {
        let metadata = fs::metadata(&location).map_err(ReadFileError::io)?;
        let bom = original.starts_with(UTF8_BOM);
        Self::new_with_bom(metadata, original, bom)
    }

    fn new_with_bom(
        metadata: fs::Metadata,
        original: Vec<u8>,
        bom: bool,
    ) -> Result<Self, ReadFileError> {
        let bytes =
            if bom { original.strip_prefix(UTF8_BOM).unwrap_or(&original) } else { &original };
        if bytes.iter().any(|byte| matches!(byte, 0..=8 | 11 | 14..=31 | 127)) {
            return Err(ReadFileError::binary());
        }
        let text = std::str::from_utf8(bytes).map_err(ReadFileError::invalid_utf8)?;
        let mut content = String::with_capacity(text.len());
        let mut endings = Vec::new();
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '\r' => {
                    let ending = if chars.peek() == Some(&'\n') {
                        chars.next();
                        LineEnding::crlf()
                    } else {
                        LineEnding::cr()
                    };
                    endings.push(ending);
                    content.push('\n');
                },
                '\n' => {
                    endings.push(LineEnding::lf());
                    content.push('\n');
                },
                _ => content.push(ch),
            }
        }
        let line_ending = endings.first().copied().unwrap_or_else(LineEnding::lf);
        Ok(Self {
            metadata,
            original: original.into(),
            content: content.into(),
            bom,
            endings,
            line_ending,
        })
    }

    fn encode(&self, content: &str) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(content.len() + self.endings.len() + UTF8_BOM.len());
        if self.bom {
            bytes.extend_from_slice(UTF8_BOM);
        }
        let content = content.replace("\r\n", "\n").replace('\r', "\n");
        let mut endings =
            vec![self.line_ending; content.bytes().filter(|byte| *byte == b'\n').count()];
        if self.endings.iter().any(|ending| *ending != self.line_ending) {
            let original = self.content.replace("\r\n", "\n").replace('\r', "\n");
            let diff = similar::TextDiff::configure()
                .timeout(Duration::from_millis(200))
                .diff_lines(&original, &content);
            for op in diff.ops() {
                for (old, new) in op.old_range().zip(op.new_range()) {
                    if let Some(ending) = self.endings.get(old)
                        && let Some(destination) = endings.get_mut(new)
                    {
                        *destination = *ending;
                    }
                }
            }
        }
        for (ix, line) in content.split_inclusive('\n').enumerate() {
            if let Some(line) = line.strip_suffix('\n') {
                bytes.extend_from_slice(line.as_bytes());
                bytes.extend_from_slice(endings[ix].text().as_bytes());
            } else {
                bytes.extend_from_slice(line.as_bytes());
            }
        }
        bytes
    }

    pub fn ending_label(&self) -> &'static str {
        if self.endings.iter().any(|ending| *ending != self.line_ending) {
            "混合换行"
        } else {
            self.line_ending.label()
        }
    }
}

pub fn read_text_file(path: &Path) -> Result<FileDocument, ReadFileError> {
    let location = fs::canonicalize(path).map_err(ReadFileError::io)?;
    let file = open_regular_file(&location)?;
    let metadata = file.metadata().map_err(ReadFileError::io)?;
    if metadata.len() > MAX_FILE_BYTES {
        return Err(ReadFileError::too_large());
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes).map_err(ReadFileError::io)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ReadFileError::too_large());
    }
    let mut document = FileDocument::new(path.to_owned(), location, bytes)?;
    document.metadata = metadata;
    Ok(document)
}

#[derive(Debug, Error)]
pub enum SaveFileError {
    #[error("保存失败，草稿仍保留：{}：{source}", path.display())]
    Io { path: PathBuf, source: io::Error },
    #[error("文件已被外部修改或移动，未覆盖。草稿仍保留，请复制草稿并核对磁盘文件。")]
    Conflict,
    #[error("文件为只读，未保存。草稿仍保留，请检查文件权限。")]
    Readonly,
    #[error("无法检查磁盘文件，未保存。草稿仍保留：{0}")]
    Read(#[source] ReadFileError),
}

impl SaveFileError {
    fn io(path: &Path, source: io::Error) -> Self {
        Self::Io { path: path.to_owned(), source }
    }

    fn conflict() -> Self {
        Self::Conflict
    }

    fn readonly() -> Self {
        Self::Readonly
    }

    fn read(error: ReadFileError) -> Self {
        Self::Read(error)
    }
}

fn same_file_identity(first: &fs::Metadata, second: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        first.dev() == second.dev() && first.ino() == second.ino()
    }
    #[cfg(not(unix))]
    {
        first.created().is_ok_and(|created| second.created().is_ok_and(|other| created == other))
    }
}

fn same_file_version(first: &fs::Metadata, second: &fs::Metadata) -> bool {
    if !same_file_identity(first, second)
        || first.len() != second.len()
        || first.modified().ok() != second.modified().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        first.ctime() == second.ctime() && first.ctime_nsec() == second.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn check_original(path: &Path, original: &[u8]) -> Result<fs::Metadata, SaveFileError> {
    let file = open_regular_file(path).map_err(SaveFileError::read)?;
    let before = file.metadata().map_err(|error| SaveFileError::io(path, error))?;
    if before.len() != original.len() as u64 {
        return Err(SaveFileError::conflict());
    }
    let mut bytes = Vec::with_capacity(original.len());
    (&file)
        .take(original.len() as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| SaveFileError::io(path, error))?;
    let after = file.metadata().map_err(|error| SaveFileError::io(path, error))?;
    let current = fs::symlink_metadata(path).map_err(|error| SaveFileError::io(path, error))?;
    if bytes != original
        || !current.is_file()
        || !same_file_version(&before, &after)
        || !same_file_version(&after, &current)
    {
        return Err(SaveFileError::conflict());
    }
    Ok(current)
}

fn create_temporary(location: &Path) -> Result<(PathBuf, fs::File), SaveFileError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    for _ in 0..16 {
        let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = format!(".aries-save-{}-{sequence}.tmp", std::process::id());
        let temporary = location.with_file_name(name);
        match options.open(&temporary) {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(SaveFileError::io(&temporary, error)),
        }
    }
    Err(SaveFileError::io(
        location,
        io::Error::new(io::ErrorKind::AlreadyExists, "无法创建唯一的保存临时文件"),
    ))
}

pub fn save_document(
    path: &Path,
    document: &FileDocument,
    content: SharedString,
) -> Result<FileDocument, SaveFileError> {
    let location = fs::canonicalize(path).map_err(|error| SaveFileError::io(path, error))?;
    let metadata = check_original(&location, &document.original)?;
    if !same_file_identity(&document.metadata, &metadata) {
        return Err(SaveFileError::conflict());
    }
    if metadata.permissions().readonly() {
        return Err(SaveFileError::readonly());
    }
    let bytes = document.encode(&content);
    let mut saved = FileDocument::new_with_bom(metadata.clone(), bytes, document.bom)
        .map_err(SaveFileError::read)?;
    saved.content = content;
    let (temporary, mut file) = create_temporary(&location)?;
    let result = (|| {
        #[cfg(target_os = "macos")]
        fs::copy(&location, &temporary).map_err(|error| SaveFileError::io(&temporary, error))?;
        file.set_len(0).map_err(|error| SaveFileError::io(&temporary, error))?;
        file.write_all(&saved.original).map_err(|error| SaveFileError::io(&temporary, error))?;
        file.set_permissions(metadata.permissions())
            .map_err(|error| SaveFileError::io(&temporary, error))?;
        file.sync_all().map_err(|error| SaveFileError::io(&temporary, error))?;
        let saved_metadata =
            file.metadata().map_err(|error| SaveFileError::io(&temporary, error))?;
        drop(file);
        let current = check_original(&location, &document.original)?;
        if !same_file_version(&metadata, &current)
            || fs::canonicalize(path).map_err(|error| SaveFileError::io(path, error))? != location
        {
            return Err(SaveFileError::conflict());
        }
        fs::rename(&temporary, &location).map_err(|error| SaveFileError::io(&location, error))?;
        Ok(saved_metadata)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    saved.metadata = result?;
    Ok(saved)
}
