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

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Preferences {
    appearance: AppearanceSettings,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
struct AppearanceSettings {
    theme: Appearance,
    zoom: f32,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self { theme: Appearance::System, zoom: 1.0 }
    }
}

impl Preferences {
    pub fn appearance(&self) -> Appearance {
        self.appearance.theme
    }

    pub fn set_appearance(&mut self, appearance: Appearance) {
        self.appearance.theme = appearance;
    }

    pub fn zoom(&self) -> f32 {
        self.appearance.zoom
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.appearance.zoom = zoom.clamp(0.8, 1.5);
    }

    pub fn path() -> Option<PathBuf> {
        dirs::home_dir().map(|path| path.join(".local/share/aries-orbit/settings.toml"))
    }

    pub fn legacy_path() -> Option<PathBuf> {
        dirs::config_dir().map(|path| path.join("aries-orbit/preferences.json"))
    }

    pub fn load(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let text = fs::read_to_string(path)?;
        let settings: Self = toml::from_str(&text)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
        if !settings.zoom().is_finite() || !(0.8..=1.5).contains(&settings.zoom()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "缩放比例必须在 0.8–1.5 之间",
            ));
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
