use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

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
        dirs::home_dir().map(|path| path.join(".local/share/aries-orbit/settings.toml"))
    }

    pub fn legacy_path() -> Option<PathBuf> {
        dirs::config_dir().map(|path| path.join("aries-orbit/preferences.json"))
    }

    pub fn load(path: impl AsRef<Path>) -> std::io::Result<Self> {
        Self::parse(&fs::read_to_string(path)?)
    }

    fn parse(text: &str) -> std::io::Result<Self> {
        let settings: Self = toml::from_str(text)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        if !settings.font_size().is_finite() || !(12.0..=24.0).contains(&settings.font_size()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "字号必须在 12–24 之间",
            ));
        }
        if settings.font_family().is_some_and(|family| family.trim().is_empty()) {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "字体名称不能为空"));
        }
        Ok(settings)
    }

    pub fn load_or_migrate(path: &Path, legacy: Option<&Path>) -> (Self, Option<String>) {
        match Self::load(path) {
            Ok(settings) => return (settings, None),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return (
                    Self::default(),
                    Some(format!("无法读取设置，已使用默认值；原文件未修改：{error}")),
                );
            }
        }
        let Some(legacy) = legacy else {
            return (Self::default(), None);
        };
        #[derive(Deserialize)]
        struct LegacyPreferences {
            appearance: Appearance,
        }
        let migrated = (|| {
            let text = fs::read_to_string(legacy)?;
            let old: LegacyPreferences = serde_json::from_str(&text)
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
            let mut settings = Self::default();
            settings.set_appearance(old.appearance);
            Ok::<_, std::io::Error>(settings)
        })();
        match migrated {
            Ok(settings) => {
                let error = settings
                    .save(path)
                    .err()
                    .map(|error| format!("旧设置已加载，但迁移保存失败：{error}"));
                (settings, error)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (Self::default(), None),
            Err(error) => (Self::default(), Some(format!("无法读取旧设置，已使用默认值：{error}"))),
        }
    }

    pub fn save(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = path.as_ref();
        let text = toml::to_string_pretty(self)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        let result = fs::write(&temporary, text).and_then(|_| fs::rename(&temporary, path));
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}
