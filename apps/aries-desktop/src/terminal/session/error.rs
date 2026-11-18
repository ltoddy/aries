use crate::terminal::session::*;

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error(transparent)]
    InvalidDimensions(#[from] InvalidDimensions),
    #[error("启动终端工作线程失败：{source}")]
    WorkerSpawn { source: std::io::Error },
}

impl StartError {
    pub fn worker_spawn(source: std::io::Error) -> Self {
        Self::WorkerSpawn { source }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("终端输入不能超过 64 KiB（当前为 {size} 字节）")]
    TooLarge { size: usize },
    #[error("终端未运行")]
    NotRunning,
    #[error("终端输入队列已满，请稍后重试")]
    QueueFull,
    #[error("终端输入队列已关闭，请重启 shell")]
    QueueClosed,
    #[error(transparent)]
    Lock(#[from] LockError),
}

impl WriteError {
    pub fn too_large(size: usize) -> Self {
        Self::TooLarge { size }
    }
    pub fn not_running() -> Self {
        Self::NotRunning
    }
    pub fn queue_full() -> Self {
        Self::QueueFull
    }
    pub fn queue_closed() -> Self {
        Self::QueueClosed
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ResizeError {
    #[error(transparent)]
    InvalidDimensions(#[from] InvalidDimensions),
    #[error("终端尺寸调整队列已满，请稍后重试")]
    QueueFull,
    #[error("终端尺寸调整队列已关闭，请重启 shell")]
    QueueClosed,
}

impl ResizeError {
    pub fn queue_full() -> Self {
        Self::QueueFull
    }
    pub fn queue_closed() -> Self {
        Self::QueueClosed
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorkerError {
    #[cfg(not(unix))]
    #[error("terminal PTY worker currently requires Unix")]
    UnsupportedPlatform,
    #[cfg(unix)]
    #[error("terminal directory is not a directory: {path}", path = .0.display())]
    NotDirectory(PathBuf),
    #[cfg(unix)]
    #[error("PTY has no file descriptor")]
    MissingFileDescriptor,
    #[cfg(unix)]
    #[error("shell has no process identifier")]
    MissingProcessId,
    #[cfg(unix)]
    #[error("terminal response queue overflow")]
    ResponseQueueOverflow,
    #[cfg(unix)]
    #[error("PTY write returned zero")]
    WriteZero,
    #[error(transparent)]
    Lock(#[from] LockError),
    #[cfg(unix)]
    #[error("terminal listener lock poisoned")]
    ListenerLockPoisoned,
    #[cfg(unix)]
    #[error("resolving terminal directory {path}: {source}", path = .path.display())]
    ResolveDirectory { path: PathBuf, source: std::io::Error },
    #[cfg(unix)]
    #[error("opening PTY: {source}")]
    OpenPty { source: Box<dyn std::error::Error + Send + Sync> },
    #[cfg(unix)]
    #[error("cloning PTY reader: {source}")]
    CloneReader { source: Box<dyn std::error::Error + Send + Sync> },
    #[cfg(unix)]
    #[error("taking PTY writer: {source}")]
    TakeWriter { source: Box<dyn std::error::Error + Send + Sync> },
    #[cfg(unix)]
    #[error("setting PTY nonblocking mode: {source}")]
    Nonblocking { source: std::io::Error },
    #[cfg(unix)]
    #[error("starting shell {shell} in {directory}: {source}", shell = .shell.display(), directory = .directory.display())]
    SpawnShell {
        shell: PathBuf,
        directory: PathBuf,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[cfg(unix)]
    #[error("waiting for shell without process identifier: {source}")]
    WaitWithoutProcessId { source: std::io::Error },
    #[cfg(unix)]
    #[error("resizing PTY: {source}")]
    ResizePty { source: Box<dyn std::error::Error + Send + Sync> },
    #[cfg(unix)]
    #[error("writing PTY input: {source}")]
    WriteInput { source: std::io::Error },
    #[cfg(unix)]
    #[error("reading PTY output: {source}")]
    ReadOutput { source: std::io::Error },
    #[cfg(unix)]
    #[error("checking shell exit status: {source}")]
    CheckExit { source: std::io::Error },
    #[cfg(unix)]
    #[error("waiting for shell: {source}")]
    WaitShell { source: std::io::Error },
}

impl WorkerError {
    #[cfg(not(unix))]
    pub fn unsupported_platform() -> Self {
        Self::UnsupportedPlatform
    }
    #[cfg(unix)]
    pub fn missing_file_descriptor() -> Self {
        Self::MissingFileDescriptor
    }
    #[cfg(unix)]
    pub fn missing_process_id() -> Self {
        Self::MissingProcessId
    }
    #[cfg(unix)]
    pub fn response_queue_overflow() -> Self {
        Self::ResponseQueueOverflow
    }
    #[cfg(unix)]
    pub fn write_zero() -> Self {
        Self::WriteZero
    }
    #[cfg(unix)]
    pub fn listener_lock_poisoned() -> Self {
        Self::ListenerLockPoisoned
    }
    #[cfg(unix)]
    pub fn not_directory(path: PathBuf) -> Self {
        Self::NotDirectory(path)
    }
    #[cfg(unix)]
    pub fn resolve_directory(path: PathBuf, source: std::io::Error) -> Self {
        Self::ResolveDirectory { path, source }
    }
    #[cfg(unix)]
    pub fn open_pty(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::OpenPty { source }
    }
    #[cfg(unix)]
    pub fn clone_reader(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::CloneReader { source }
    }
    #[cfg(unix)]
    pub fn take_writer(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::TakeWriter { source }
    }
    #[cfg(unix)]
    pub fn nonblocking(source: std::io::Error) -> Self {
        Self::Nonblocking { source }
    }
    #[cfg(unix)]
    pub fn spawn_shell(
        shell: PathBuf,
        directory: PathBuf,
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        Self::SpawnShell { shell, directory, source }
    }
    #[cfg(unix)]
    pub fn wait_without_process_id(source: std::io::Error) -> Self {
        Self::WaitWithoutProcessId { source }
    }
    #[cfg(unix)]
    pub fn resize_pty(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::ResizePty { source }
    }
    #[cfg(unix)]
    pub fn write_input(source: std::io::Error) -> Self {
        Self::WriteInput { source }
    }
    #[cfg(unix)]
    pub fn read_output(source: std::io::Error) -> Self {
        Self::ReadOutput { source }
    }
    #[cfg(unix)]
    pub fn check_exit(source: std::io::Error) -> Self {
        Self::CheckExit { source }
    }
    #[cfg(unix)]
    pub fn wait_shell(source: std::io::Error) -> Self {
        Self::WaitShell { source }
    }
}
