use std::path::PathBuf;
use std::{env, io};

#[derive(Debug, Clone)]
pub struct GlobalContext {
    pub current_dir: PathBuf,
    pub home_dir: PathBuf,
    pub config_dir: PathBuf,
    pub user: String,
}

impl GlobalContext {
    pub fn new() -> io::Result<Self> {
        let current_dir = env::current_dir()?;
        let home_dir = env::home_dir()
            .ok_or(io::Error::new(io::ErrorKind::NotADirectory, "无法识别 home 目录"))?;
        let config_dir = home_dir.join(".local").join("share").join("aries");

        if !config_dir.exists() {
            std::fs::create_dir_all(&config_dir)?;
        }

        let user = whoami::realname().unwrap_or_default();

        Ok(Self { current_dir, home_dir, config_dir, user })
    }

    pub fn with_current_dir(current_dir: PathBuf) -> io::Result<Self> {
        let home_dir = env::home_dir()
            .ok_or(io::Error::new(io::ErrorKind::NotADirectory, "无法识别 home 目录"))?;
        let config_dir = home_dir.join(".local").join("share").join("aries");

        if !config_dir.exists() {
            std::fs::create_dir_all(&config_dir)?;
        }

        let user = whoami::realname().unwrap_or_default();

        Ok(Self { current_dir, home_dir, config_dir, user })
    }
}
