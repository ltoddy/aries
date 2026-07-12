use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct GlobalContext {
    pub home_dir: PathBuf,
    pub root_dir: PathBuf,
    pub user: String,
}

impl GlobalContext {
    pub fn new() -> io::Result<Self> {
        let home_dir = dirs::home_dir()
            .ok_or(io::Error::new(io::ErrorKind::NotFound, "Impossible to get your home dir!"))?;

        let root_dir = home_dir.join(".local").join("share").join("aries");
        std::fs::create_dir_all(&root_dir)?;

        let user = whoami::realname().unwrap_or_default();

        Ok(Self { home_dir, root_dir, user })
    }
}
