use std::fs::{self, File, OpenOptions, Permissions, TryLockError};
use std::io;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use tokio::net::{UnixListener, UnixStream};

pub struct Socket {
    file_path: PathBuf,
    _lock: File,
}

impl Socket {
    pub async fn bind(root_dir: impl AsRef<Path>) -> io::Result<(Self, UnixListener)> {
        let root_dir = root_dir.as_ref();

        fs::DirBuilder::new().recursive(true).mode(0o700).create(root_dir)?;
        fs::set_permissions(root_dir, Permissions::from_mode(0o700))?;

        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(root_dir.join("aries.lock"))?;
        lock.try_lock().map_err(|err| match err {
            TryLockError::WouldBlock => {
                io::Error::new(io::ErrorKind::AddrInUse, "another Aries IPC server is running")
            },
            TryLockError::Error(source) => source,
        })?;

        let file_path = root_dir.join("aries.sock");
        match fs::symlink_metadata(&file_path) {
            Ok(metadata) => {
                if !metadata.file_type().is_socket() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "socket path exists but is not a Unix socket",
                    ));
                }
                match UnixStream::connect(&file_path).await {
                    Ok(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::AddrInUse,
                            "another server is listening on the Aries socket",
                        ));
                    },
                    Err(err) if err.kind() == io::ErrorKind::ConnectionRefused => {
                        fs::remove_file(&file_path)?;
                    },
                    Err(err) if err.kind() == io::ErrorKind::NotFound => {},
                    Err(err) => return Err(err),
                }
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => {},
            Err(err) => return Err(err),
        }

        let listener = UnixListener::bind(&file_path)?;
        let socket = Self::new(file_path, lock);
        fs::set_permissions(&socket.file_path, Permissions::from_mode(0o600))?;
        Ok((socket, listener))
    }

    fn new(path: impl Into<PathBuf>, lock: File) -> Self {
        let path = path.into();

        Self { file_path: path, _lock: lock }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        if let Err(err) = fs::remove_file(&self.file_path)
            && err.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(path = %self.file_path.display(), error = %err, "failed to remove Unix socket");
        }
    }
}
