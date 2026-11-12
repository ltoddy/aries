use std::path::{Path, PathBuf};
use std::{fs, io};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidatePreferencesError {
    #[error("字号必须在 12–24 之间，当前值为 {size}")]
    FontSize { size: f32 },
    #[error("字体名称不能为空")]
    FontFamily,
}

impl ValidatePreferencesError {
    pub fn font_size(size: f32) -> Self {
        Self::FontSize { size }
    }

    pub fn font_family() -> Self {
        Self::FontFamily
    }
}

#[derive(Debug, Error)]
pub enum LoadPreferencesError {
    #[error("无法读取设置文件 {}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("无法解析设置文件 {}", path.display())]
    Toml { path: PathBuf, source: toml::de::Error },
    #[error("设置文件 {} 无效", path.display())]
    Validation { path: PathBuf, source: ValidatePreferencesError },
}

impl LoadPreferencesError {
    pub fn read(path: &Path, source: io::Error) -> Self {
        Self::Read { path: path.to_owned(), source }
    }

    pub fn toml(path: &Path, source: toml::de::Error) -> Self {
        Self::Toml { path: path.to_owned(), source }
    }

    pub fn validation(path: &Path, source: ValidatePreferencesError) -> Self {
        Self::Validation { path: path.to_owned(), source }
    }
}

#[derive(Debug, Error)]
pub enum SavePreferencesError {
    #[error("未找到用户主目录，设置无法保存")]
    MissingHome,
    #[error("无法序列化设置到 {}", path.display())]
    Serialize { path: PathBuf, source: toml::ser::Error },
    #[error("无法创建设置目录 {}", path.display())]
    CreateDirectory { path: PathBuf, source: io::Error },
    #[error("无法写入临时设置文件 {}", path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error("无法将临时设置文件 {} 替换到 {}", temporary.display(), path.display())]
    Rename { temporary: PathBuf, path: PathBuf, source: io::Error },
}

impl SavePreferencesError {
    pub fn missing_home() -> Self {
        Self::MissingHome
    }

    pub fn serialize(path: &Path, source: toml::ser::Error) -> Self {
        Self::Serialize { path: path.to_owned(), source }
    }

    pub fn create_directory(path: &Path, source: io::Error) -> Self {
        Self::CreateDirectory { path: path.to_owned(), source }
    }

    pub fn write(path: &Path, source: io::Error) -> Self {
        Self::Write { path: path.to_owned(), source }
    }

    pub fn rename(temporary: &Path, path: &Path, source: io::Error) -> Self {
        Self::Rename { temporary: temporary.to_owned(), path: path.to_owned(), source }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SendKey {
    #[default]
    Enter,
    CommandEnter,
}

impl SendKey {
    pub fn send_label(self) -> &'static str {
        match self {
            Self::Enter => "Enter",
            Self::CommandEnter => {
                if cfg!(target_os = "macos") {
                    "Command + Enter"
                } else {
                    "Ctrl + Enter"
                }
            },
        }
    }

    pub fn newline_label(self) -> &'static str {
        match self {
            Self::Enter => Self::CommandEnter.send_label(),
            Self::CommandEnter => "Enter",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Preferences {
    appearance: AppearanceSettings,
    send_key: SendKey,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
struct AppearanceSettings {
    theme: Appearance,
    font_family: Option<String>,
    font_size: f32,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self { theme: Appearance::System, font_family: None, font_size: 16.0 }
    }
}

impl Preferences {
    pub fn send_key(&self) -> SendKey {
        self.send_key
    }

    pub fn set_send_key(&mut self, send_key: SendKey) {
        self.send_key = send_key;
    }

    pub fn appearance(&self) -> Appearance {
        self.appearance.theme
    }

    pub fn set_appearance(&mut self, appearance: Appearance) {
        self.appearance.theme = appearance;
    }

    pub fn font_family(&self) -> Option<&str> {
        self.appearance.font_family.as_deref()
    }

    pub fn set_font_family(&mut self, family: Option<String>) {
        self.appearance.font_family = family;
    }

    pub fn font_size(&self) -> f32 {
        self.appearance.font_size
    }

    pub fn set_font_size(&mut self, size: f32) {
        self.appearance.font_size = size.clamp(12.0, 24.0);
    }

    pub fn path() -> Option<PathBuf> {
        dirs::home_dir().map(|path| path.join(".local/share/aries-desktop/settings.toml"))
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, LoadPreferencesError> {
        let path = path.as_ref();
        let text =
            fs::read_to_string(path).map_err(|source| LoadPreferencesError::read(path, source))?;
        let settings: Self =
            toml::from_str(&text).map_err(|source| LoadPreferencesError::toml(path, source))?;
        settings.validate().map_err(|source| LoadPreferencesError::validation(path, source))?;
        Ok(settings)
    }

    fn validate(&self) -> Result<(), ValidatePreferencesError> {
        let size = self.font_size();
        if !size.is_finite() || !(12.0..=24.0).contains(&size) {
            return Err(ValidatePreferencesError::font_size(size));
        }
        if self.font_family().is_some_and(|family| family.trim().is_empty()) {
            return Err(ValidatePreferencesError::font_family());
        }
        Ok(())
    }

    pub fn load_or_default(path: impl AsRef<Path>) -> (Self, Option<LoadPreferencesError>) {
        match Self::load(path) {
            Ok(settings) => (settings, None),
            Err(LoadPreferencesError::Read { source, .. })
                if source.kind() == io::ErrorKind::NotFound =>
            {
                (Self::default(), None)
            },
            Err(error) => (Self::default(), Some(error)),
        }
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), SavePreferencesError> {
        let path = path.as_ref();
        let text = toml::to_string_pretty(self)
            .map_err(|source| SavePreferencesError::serialize(path, source))?;
        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            fs::create_dir_all(parent)
                .map_err(|source| SavePreferencesError::create_directory(parent, source))?;
        }
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        let result = fs::write(&temporary, text)
            .map_err(|source| SavePreferencesError::write(&temporary, source))
            .and_then(|_| {
                fs::rename(&temporary, path)
                    .map_err(|source| SavePreferencesError::rename(&temporary, path, source))
            });
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}
