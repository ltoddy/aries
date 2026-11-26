use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum PetError {
    #[error("未找到用户主目录，无法加载宠物")]
    MissingHome,
    #[error("无法访问宠物文件 {}", path.display())]
    Io { path: PathBuf, source: io::Error },
    #[error("无法解析宠物文件 {}", path.display())]
    Metadata { path: PathBuf, source: serde_json::Error },
    #[error("宠物素材不受支持：{reason}")]
    Unsupported { reason: String },
    #[error("无法解码内置宠物素材")]
    Image {
        #[source]
        source: image::ImageError,
    },
    #[error("无法创建宠物窗口：{reason}")]
    Window { reason: String },
    #[error("无法操作 Aries 原生窗口：{reason}")]
    Native { reason: String },
    #[cfg(target_os = "macos")]
    #[error("无法读取 Aries 原生窗口句柄")]
    NativeHandle { source: raw_window_handle::HandleError },
}

impl PetError {
    pub fn missing_home() -> Self {
        Self::MissingHome
    }

    pub fn io(path: &Path, source: io::Error) -> Self {
        Self::Io { path: path.to_owned(), source }
    }

    pub fn metadata(path: &Path, source: serde_json::Error) -> Self {
        Self::Metadata { path: path.to_owned(), source }
    }

    pub fn unsupported(reason: String) -> Self {
        Self::Unsupported { reason }
    }

    pub fn image(source: image::ImageError) -> Self {
        Self::Image { source }
    }

    pub fn window(reason: String) -> Self {
        Self::Window { reason }
    }

    pub fn native(reason: String) -> Self {
        Self::Native { reason }
    }

    #[cfg(target_os = "macos")]
    pub fn native_handle(source: raw_window_handle::HandleError) -> Self {
        Self::NativeHandle { source }
    }
}
