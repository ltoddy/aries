use std::fs::create_dir_all;
use std::path::PathBuf;
use std::{env, io};

#[derive(Debug, Clone)]
pub struct GlobalContext {
    pub current_dir: PathBuf,
    pub home_dir: PathBuf,
    pub config_dir: PathBuf,
}

impl GlobalContext {
    pub fn new() -> io::Result<Self> {
        let current_dir = env::current_dir()?;
        let home_dir = env::home_dir().ok_or(io::Error::new(io::ErrorKind::NotADirectory, "无法识别 home 目录"))?;
        let config_dir = home_dir.join(".local").join("share").join("aries");

        if let Some(parent) = config_dir.parent() {
            create_dir_all(parent)?;
        }

        Ok(Self { current_dir, home_dir, config_dir })
    }
}
