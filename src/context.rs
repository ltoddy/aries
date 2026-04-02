use std::path::PathBuf;

use directories::UserDirs;

use crate::config::AppConfig;
use crate::theme::{Theme, detect};

#[derive(Clone)]
pub struct GlobalContext {
    pub config: AppConfig,
    pub config_dir: PathBuf,
    pub current_dir: PathBuf,
    #[allow(dead_code)]
    pub home_dir: PathBuf,
    pub theme: Theme,
}

impl GlobalContext {
    pub fn new(config: AppConfig, current_dir: PathBuf, config_dir: PathBuf) -> anyhow::Result<Self> {
        let home_dir = UserDirs::new().map(|dirs| dirs.home_dir().to_path_buf()).unwrap_or_else(|| PathBuf::from("~"));
        let theme = detect();

        Ok(Self { config, config_dir, current_dir, home_dir, theme })
    }
}
