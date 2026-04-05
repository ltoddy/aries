use std::path::PathBuf;
use std::{env, io};

use colored::{Color, Colorize};
use serde::{Deserialize, Serialize};
use tokio::fs::create_dir_all;

#[derive(Debug, Clone)]
pub struct GlobalContext {
    pub current_dir: PathBuf,
    pub home_dir: PathBuf,
    pub config_dir: PathBuf,
    pub theme: Theme,
    pub config: AppConfig,
}

impl GlobalContext {
    pub async fn new(config: AppConfig) -> io::Result<Self> {
        let current_dir = env::current_dir()?;
        let home_dir = env::home_dir().ok_or(io::Error::new(io::ErrorKind::NotADirectory, "无法识别 home 目录"))?;
        let config_dir = home_dir.join(".local").join("share").join("aries");

        if let Some(parent) = config_dir.parent() {
            create_dir_all(parent).await?;
        }

        let theme = Theme::default();

        Ok(Self { current_dir, home_dir, config_dir, theme, config })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

#[derive(Debug, Clone, Copy)]
pub enum Theme {
    Light,
    Dark,
}

impl Default for Theme {
    fn default() -> Self {
        match dark_light::detect() {
            Ok(dark_light::Mode::Dark) => Theme::Dark,
            _ => Theme::Light,
        }
    }
}

#[allow(dead_code)]
impl Theme {
    pub fn is_light(&self) -> bool {
        matches!(self, Theme::Light)
    }

    pub fn is_dark(&self) -> bool {
        matches!(self, Theme::Dark)
    }

    pub fn cyan(&self) -> Color {
        if self.is_dark() { Color::BrightCyan } else { Color::Cyan }
    }

    pub fn blue(&self) -> Color {
        if self.is_dark() { Color::BrightBlue } else { Color::Blue }
    }

    pub fn magenta(&self) -> Color {
        if self.is_dark() { Color::BrightMagenta } else { Color::Magenta }
    }

    pub fn red(&self) -> Color {
        if self.is_dark() { Color::BrightRed } else { Color::Red }
    }

    pub fn green(&self) -> Color {
        if self.is_dark() { Color::BrightGreen } else { Color::Green }
    }

    pub fn yellow(&self) -> Color {
        if self.is_dark() { Color::BrightYellow } else { Color::Yellow }
    }

    pub fn white(&self) -> Color {
        if self.is_dark() { Color::BrightWhite } else { Color::White }
    }

    pub fn black(&self) -> Color {
        if self.is_dark() { Color::BrightBlack } else { Color::Black }
    }

    pub fn dimmed(&self, s: &str) -> colored::ColoredString {
        if self.is_dark() { s.bright_black() } else { s.dimmed() }
    }

    pub fn cyan_text(&self, s: &str) -> colored::ColoredString {
        s.color(self.cyan())
    }

    pub fn yellow_text(&self, s: &str) -> colored::ColoredString {
        s.color(self.yellow())
    }

    pub fn green_text(&self, s: &str) -> colored::ColoredString {
        s.color(self.green())
    }

    pub fn red_text(&self, s: &str) -> colored::ColoredString {
        s.color(self.red())
    }

    pub fn blue_text(&self, s: &str) -> colored::ColoredString {
        s.color(self.blue())
    }
}
