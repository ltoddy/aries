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

#[derive(Debug, Error)]
pub enum LoadPreferencesError {
    #[error("无法读取设置文件 {}：{source}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("无法解析设置文件 {}：{source}", path.display())]
    Toml { path: PathBuf, source: toml::de::Error },
    #[error("设置文件 {} 无效：{source}", path.display())]
    Validation { path: PathBuf, source: ValidatePreferencesError },
}

#[derive(Debug, Error)]
pub enum SavePreferencesError {
    #[error("无法序列化设置到 {}：{source}", path.display())]
    Serialize { path: PathBuf, source: toml::ser::Error },
    #[error("无法创建设置目录 {}：{source}", path.display())]
    CreateDirectory { path: PathBuf, source: io::Error },
    #[error("无法写入临时设置文件 {}：{source}", path.display())]
    Write { path: PathBuf, source: io::Error },
    #[error("无法将临时设置文件 {} 替换到 {}：{source}", temporary.display(), path.display())]
    Rename { temporary: PathBuf, path: PathBuf, source: io::Error },
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
    ShiftEnter,
}

impl SendKey {
    pub fn send_label(self) -> &'static str {
        match self {
            Self::Enter => "Enter",
            Self::ShiftEnter => "Shift + Enter",
        }
    }

    pub fn newline_label(self) -> &'static str {
        match self {
            Self::Enter => "Shift + Enter",
            Self::ShiftEnter => "Enter",
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
        let text = fs::read_to_string(path)
            .map_err(|source| LoadPreferencesError::Read { path: path.to_owned(), source })?;
        let settings: Self = toml::from_str(&text)
            .map_err(|source| LoadPreferencesError::Toml { path: path.to_owned(), source })?;
        settings
            .validate()
            .map_err(|source| LoadPreferencesError::Validation { path: path.to_owned(), source })?;
        Ok(settings)
    }

    fn validate(&self) -> Result<(), ValidatePreferencesError> {
        let size = self.font_size();
        if !size.is_finite() || !(12.0..=24.0).contains(&size) {
            return Err(ValidatePreferencesError::FontSize { size });
        }
        if self.font_family().is_some_and(|family| family.trim().is_empty()) {
            return Err(ValidatePreferencesError::FontFamily);
        }
        Ok(())
    }

    pub fn load_or_default(path: &Path) -> (Self, Option<String>) {
        match Self::load(path) {
            Ok(settings) => (settings, None),
            Err(LoadPreferencesError::Read { source, .. })
                if source.kind() == io::ErrorKind::NotFound =>
            {
                (Self::default(), None)
            },
            Err(error) => (
                Self::default(),
                Some(format!("无法读取设置，已使用默认值；原文件未修改：{error}")),
            ),
        }
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), SavePreferencesError> {
        let path = path.as_ref();
        let text = toml::to_string_pretty(self)
            .map_err(|source| SavePreferencesError::Serialize { path: path.to_owned(), source })?;
        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|source| SavePreferencesError::CreateDirectory {
                path: parent.to_owned(),
                source,
            })?;
        }
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        let result = fs::write(&temporary, text)
            .map_err(|source| SavePreferencesError::Write { path: temporary.to_owned(), source })
            .and_then(|_| {
                fs::rename(&temporary, path).map_err(|source| SavePreferencesError::Rename {
                    temporary: temporary.to_owned(),
                    path: path.to_owned(),
                    source,
                })
            });
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}
