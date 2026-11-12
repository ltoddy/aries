use std::collections::VecDeque;
#[cfg(unix)]
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, mpsc};
use std::thread;
#[cfg(unix)]
use std::time::{Duration, Instant};

use alacritty_terminal::Term;
use alacritty_terminal::event::{Event, EventListener, WindowSize};
use alacritty_terminal::grid::Dimensions as GridDimensions;
use alacritty_terminal::term::{Config, Osc52};
#[cfg(unix)]
use alacritty_terminal::vte::ansi::Processor;
#[cfg(unix)]
use portable_pty::{CommandBuilder, PtySize, native_pty_system};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Starting,
    Running,
    Exited { code: u32, signal: Option<String> },
    Failed(String),
}

impl Status {
    pub fn starting() -> Self {
        Self::Starting
    }
    pub fn running() -> Self {
        Self::Running
    }
    pub fn exited(code: u32, signal: Option<String>) -> Self {
        Self::Exited { code, signal }
    }
    pub fn failed(error: impl std::fmt::Display) -> Self {
        Self::Failed(error.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dimensions {
    pub columns: usize,
    pub lines: usize,
}

impl Dimensions {
    pub fn new(columns: usize, lines: usize) -> Self {
        Self { columns, lines }
    }
}

impl GridDimensions for Dimensions {
    fn total_lines(&self) -> usize {
        self.lines
    }
    fn screen_lines(&self) -> usize {
        self.lines
    }
    fn columns(&self) -> usize {
        self.columns
    }
}

#[derive(Debug, thiserror::Error)]
#[error(
    "终端尺寸超出允许范围：{columns} 列、{lines} 行（列数须为 2–1000，行数须为 1–1000）",
    columns = .dimensions.columns,
    lines = .dimensions.lines
)]
pub struct InvalidDimensions {
    pub dimensions: Dimensions,
}

impl InvalidDimensions {
    pub fn new(dimensions: Dimensions) -> Self {
        Self { dimensions }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("terminal {resource} lock poisoned")]
pub struct LockError {
    resource: &'static str,
}

impl LockError {
    pub fn new(resource: &'static str) -> Self {
        Self { resource }
    }
}

fn lock<'a, T>(
    mutex: &'a Mutex<T>,
    resource: &'static str,
) -> Result<MutexGuard<'a, T>, LockError> {
    mutex.lock().map_err(|_| LockError::new(resource))
}

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error(transparent)]
    InvalidDimensions(#[from] InvalidDimensions),
    #[error("启动终端工作线程失败：{source}")]
    WorkerSpawn { source: std::io::Error },
}

impl StartError {
    fn worker_spawn(source: std::io::Error) -> Self {
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
    fn queue_full() -> Self {
        Self::QueueFull
    }
    fn queue_closed() -> Self {
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
    fn queue_full() -> Self {
        Self::QueueFull
    }
    fn queue_closed() -> Self {
        Self::QueueClosed
    }
}

#[derive(Debug, thiserror::Error)]
enum WorkerError {
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
    fn unsupported_platform() -> Self {
        Self::UnsupportedPlatform
    }
    #[cfg(unix)]
    fn missing_file_descriptor() -> Self {
        Self::MissingFileDescriptor
    }
    #[cfg(unix)]
    fn missing_process_id() -> Self {
        Self::MissingProcessId
    }
    #[cfg(unix)]
    fn response_queue_overflow() -> Self {
        Self::ResponseQueueOverflow
    }
    #[cfg(unix)]
    fn write_zero() -> Self {
        Self::WriteZero
    }
    #[cfg(unix)]
    fn listener_lock_poisoned() -> Self {
        Self::ListenerLockPoisoned
    }
    #[cfg(unix)]
    fn not_directory(path: PathBuf) -> Self {
        Self::NotDirectory(path)
    }
    #[cfg(unix)]
    fn resolve_directory(path: PathBuf, source: std::io::Error) -> Self {
        Self::ResolveDirectory { path, source }
    }
    #[cfg(unix)]
    fn open_pty(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::OpenPty { source }
    }
    #[cfg(unix)]
    fn clone_reader(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::CloneReader { source }
    }
    #[cfg(unix)]
    fn take_writer(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::TakeWriter { source }
    }
    #[cfg(unix)]
    fn nonblocking(source: std::io::Error) -> Self {
        Self::Nonblocking { source }
    }
    #[cfg(unix)]
    fn spawn_shell(
        shell: PathBuf,
        directory: PathBuf,
        source: Box<dyn std::error::Error + Send + Sync>,
    ) -> Self {
        Self::SpawnShell { shell, directory, source }
    }
    #[cfg(unix)]
    fn wait_without_process_id(source: std::io::Error) -> Self {
        Self::WaitWithoutProcessId { source }
    }
    #[cfg(unix)]
    fn resize_pty(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::ResizePty { source }
    }
    #[cfg(unix)]
    fn write_input(source: std::io::Error) -> Self {
        Self::WriteInput { source }
    }
    #[cfg(unix)]
    fn read_output(source: std::io::Error) -> Self {
        Self::ReadOutput { source }
    }
    #[cfg(unix)]
    fn check_exit(source: std::io::Error) -> Self {
        Self::CheckExit { source }
    }
    #[cfg(unix)]
    fn wait_shell(source: std::io::Error) -> Self {
        Self::WaitShell { source }
    }
}

#[cfg_attr(not(unix), allow(dead_code))]
enum Command {
    Write(Vec<u8>),
    Resize(Dimensions, u16, u16),
}

impl Command {
    fn write(bytes: Vec<u8>) -> Self {
        Self::Write(bytes)
    }
    fn resize(dimensions: Dimensions, width: u16, height: u16) -> Self {
        Self::Resize(dimensions, width, height)
    }
}

#[derive(Clone)]
pub struct Listener {
    responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    size: Arc<Mutex<WindowSize>>,
    overflow: Arc<AtomicBool>,
    poisoned: Arc<AtomicBool>,
}

impl Listener {
    fn new(
        responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
        size: Arc<Mutex<WindowSize>>,
        overflow: Arc<AtomicBool>,
        poisoned: Arc<AtomicBool>,
    ) -> Self {
        Self { responses, size, overflow, poisoned }
    }
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let response = match event {
            Event::PtyWrite(text) => Some(text),
            Event::ColorRequest(index, format) if index < 256 => {
                Some(format(crate::terminal::paint::indexed_rgb(index)))
            },
            Event::TextAreaSizeRequest(format) => match lock(&self.size, "size") {
                Ok(size) => Some(format(*size)),
                Err(_) => {
                    self.poisoned.store(true, Ordering::Release);
                    return;
                },
            },
            _ => None,
        };
        if let Some(response) = response {
            let Ok(mut queue) = lock(&self.responses, "response") else {
                self.poisoned.store(true, Ordering::Release);
                return;
            };
            if queue.len() < 128 && response.len() <= 65536 {
                queue.push_back(response.into_bytes());
            } else {
                self.overflow.store(true, Ordering::Release);
            }
        }
    }
}

pub struct Session {
    terminal: Arc<Mutex<Term<Listener>>>,
    status: Arc<Mutex<Status>>,
    commands: mpsc::SyncSender<Command>,
    cancelled: Arc<AtomicBool>,
    completion: async_channel::Receiver<()>,
}

impl Session {
    pub fn start(
        directory: impl AsRef<Path>,
        dimensions: Dimensions,
        notifications: async_channel::Sender<()>,
    ) -> Result<Self, StartError> {
        validate_dimensions(dimensions)?;
        let responses = Arc::new(Mutex::new(VecDeque::new()));
        let overflow = Arc::new(AtomicBool::new(false));
        let poisoned = Arc::new(AtomicBool::new(false));
        let size = Arc::new(Mutex::new(WindowSize {
            num_lines: dimensions.lines as u16,
            num_cols: dimensions.columns as u16,
            cell_width: 0,
            cell_height: 0,
        }));
        let listener = Listener::new(responses, size, overflow, poisoned);
        let terminal = Arc::new(Mutex::new(Term::new(
            Config { scrolling_history: 10000, osc52: Osc52::Disabled, ..Config::default() },
            &dimensions,
            listener.clone(),
        )));
        let status = Arc::new(Mutex::new(Status::starting()));
        let cancelled = Arc::new(AtomicBool::new(false));
        let (commands, receiver) = mpsc::sync_channel(128);
        let (completed, completion) = async_channel::bounded(1);
        let session = Self {
            terminal: terminal.clone(),
            status: status.clone(),
            commands,
            cancelled: cancelled.clone(),
            completion,
        };
        let directory = directory.as_ref().to_owned();
        thread::Builder::new()
            .name("terminal-pty".into())
            .spawn(move || {
                let result = run(
                    directory,
                    dimensions,
                    Worker::new(
                        terminal,
                        status.clone(),
                        receiver,
                        cancelled,
                        listener,
                        notifications.clone(),
                    ),
                );
                if let Err(error) = result.inspect_err(|error| {
                    crate::error::log("终端工作线程失败", error);
                }) && let Ok(mut status) = lock(&status, "status")
                {
                    *status = Status::failed(error);
                }
                let _ = notifications.try_send(());
                let _ = completed.try_send(());
            })
            .map_err(StartError::worker_spawn)?;
        Ok(session)
    }

    pub async fn shutdown(self) {
        self.cancelled.store(true, Ordering::Release);
        let _ = self.completion.recv().await;
    }

    pub fn terminal(&self) -> Result<MutexGuard<'_, Term<Listener>>, LockError> {
        lock(&self.terminal, "screen")
    }

    pub fn status(&self) -> Status {
        lock(&self.status, "status").map_or_else(Status::failed, |status| status.clone())
    }

    pub fn write(&self, bytes: Vec<u8>) -> Result<(), WriteError> {
        if bytes.is_empty() {
            return Ok(());
        }
        if bytes.len() > 65536 {
            return Err(WriteError::too_large(bytes.len()));
        }
        if !matches!(*lock(&self.status, "status")?, Status::Starting | Status::Running) {
            return Err(WriteError::not_running());
        }
        self.commands.try_send(Command::write(bytes)).map_err(|error| match error {
            mpsc::TrySendError::Full(_) => WriteError::queue_full(),
            mpsc::TrySendError::Disconnected(_) => WriteError::queue_closed(),
        })
    }

    pub fn resize(
        &self,
        dimensions: Dimensions,
        cell_width: u16,
        cell_height: u16,
    ) -> Result<(), ResizeError> {
        validate_dimensions(dimensions)?;
        self.commands.try_send(Command::resize(dimensions, cell_width, cell_height)).map_err(
            |error| match error {
                mpsc::TrySendError::Full(_) => ResizeError::queue_full(),
                mpsc::TrySendError::Disconnected(_) => ResizeError::queue_closed(),
            },
        )
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

fn validate_dimensions(dimensions: Dimensions) -> Result<(), InvalidDimensions> {
    if !(2..=1000).contains(&dimensions.columns) || !(1..=1000).contains(&dimensions.lines) {
        return Err(InvalidDimensions::new(dimensions));
    }
    Ok(())
}

#[cfg(unix)]
fn shell() -> PathBuf {
    if let Some(value) = std::env::var_os("SHELL").filter(|value| !value.is_empty()) {
        return value.into();
    }
    #[cfg(unix)]
    unsafe {
        let mut storage = vec![0u8; 16384];
        let mut record: libc::passwd = std::mem::zeroed();
        let mut entry = std::ptr::null_mut();
        let result = libc::getpwuid_r(
            libc::getuid(),
            &mut record,
            storage.as_mut_ptr().cast(),
            storage.len(),
            &mut entry,
        );
        if result == 0 && !entry.is_null() && !(*entry).pw_shell.is_null() {
            let value = std::ffi::CStr::from_ptr((*entry).pw_shell).to_string_lossy();
            if !value.is_empty() {
                return PathBuf::from(value.as_ref());
            }
        }
    }
    PathBuf::from("/bin/sh")
}

#[cfg(unix)]
fn signal_group(pid: i32, group: i32, session: i32, signal: i32) {
    if group > 1
        && group != unsafe { libc::getpgrp() }
        && unsafe { libc::getsid(pid) } == session
        && unsafe { libc::getpgid(pid) } == group
    {
        unsafe {
            libc::kill(-group, signal);
        }
    }
}

#[cfg(target_os = "macos")]
fn process_alive(pid: i32) -> bool {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of_val(&info) as i32;
    unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut info as *mut libc::proc_bsdinfo).cast(),
            size,
        ) == size
            && info.pbi_status != libc::SZOMB
    }
}

#[cfg(target_os = "linux")]
fn process_alive(pid: i32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).ok().and_then(|stat| {
        stat.rsplit_once(')').map(|(_, fields)| fields.trim_start().starts_with(['Z', 'X']))
    }) == Some(false)
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "linux"))))]
fn process_alive(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

#[cfg(unix)]
fn session_processes(session: i32, deadline: Instant) -> Vec<(i32, i32)> {
    #[cfg(target_os = "macos")]
    let pids = {
        let bytes = unsafe { libc::proc_listpids(1, 0, std::ptr::null_mut(), 0) };
        let mut pids = vec![0i32; (bytes.max(0) as usize / std::mem::size_of::<i32>()) + 1024];
        let bytes = unsafe {
            libc::proc_listpids(
                1,
                0,
                pids.as_mut_ptr().cast(),
                (pids.len() * std::mem::size_of::<i32>()) as i32,
            )
        };
        pids.truncate(bytes.max(0) as usize / std::mem::size_of::<i32>());
        pids
    };
    #[cfg(target_os = "linux")]
    let pids = std::fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse::<i32>().ok());
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let pids = [session];
    let mut processes = Vec::new();
    for pid in pids {
        if Instant::now() >= deadline {
            break;
        }
        if pid > 1 && unsafe { libc::getsid(pid) } == session && process_alive(pid) {
            let group = unsafe { libc::getpgid(pid) };
            if group > 1 {
                processes.push((pid, group));
            }
        }
    }
    processes
}

#[cfg(unix)]
fn child_exited(pid: u32) -> std::io::Result<bool> {
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let result = unsafe {
        libc::waitid(libc::P_PID, pid, &mut info, libc::WEXITED | libc::WNOHANG | libc::WNOWAIT)
    };
    if result < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { info.si_pid() } != 0)
}

#[cfg_attr(not(unix), allow(dead_code))]
struct Worker {
    terminal: Arc<Mutex<Term<Listener>>>,
    status: Arc<Mutex<Status>>,
    receiver: mpsc::Receiver<Command>,
    cancelled: Arc<AtomicBool>,
    listener: Listener,
    notifications: async_channel::Sender<()>,
}

impl Worker {
    fn new(
        terminal: Arc<Mutex<Term<Listener>>>,
        status: Arc<Mutex<Status>>,
        receiver: mpsc::Receiver<Command>,
        cancelled: Arc<AtomicBool>,
        listener: Listener,
        notifications: async_channel::Sender<()>,
    ) -> Self {
        Self { terminal, status, receiver, cancelled, listener, notifications }
    }
}

#[cfg(not(unix))]
fn run(_: PathBuf, _: Dimensions, _: Worker) -> Result<(), WorkerError> {
    Err(WorkerError::unsupported_platform())
}

#[cfg(unix)]
fn run(directory: PathBuf, dimensions: Dimensions, worker: Worker) -> Result<(), WorkerError> {
    let Worker { terminal, status, receiver, cancelled, listener, notifications } = worker;
    let Listener { responses, size, overflow, poisoned } = listener;
    if cancelled.load(Ordering::Acquire) {
        *lock(&status, "status")? = Status::exited(0, None);
        return Ok(());
    }
    let directory = std::fs::canonicalize(&directory)
        .map_err(|source| WorkerError::resolve_directory(directory, source))?;
    if !directory.is_dir() {
        return Err(WorkerError::not_directory(directory));
    }
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: dimensions.lines as u16,
            cols: dimensions.columns as u16,
            ..PtySize::default()
        })
        .map_err(|source| WorkerError::open_pty(source.into()))?;
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|source| WorkerError::clone_reader(source.into()))?;
    let mut writer =
        pair.master.take_writer().map_err(|source| WorkerError::take_writer(source.into()))?;
    #[cfg(unix)]
    {
        let fd = pair.master.as_raw_fd().ok_or_else(WorkerError::missing_file_descriptor)?;
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(WorkerError::nonblocking(std::io::Error::last_os_error()));
        }
    }
    let shell = shell();
    let mut command = CommandBuilder::new(&shell);
    command.arg("-l");
    command.cwd(&directory);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    if cancelled.load(Ordering::Acquire) {
        *lock(&status, "status")? = Status::exited(0, None);
        return Ok(());
    }
    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|source| WorkerError::spawn_shell(shell, directory, source.into()))?;
    drop(pair.slave);
    let Some(pid) = child.process_id() else {
        let _ = child.kill();
        child.wait().map_err(WorkerError::wait_without_process_id)?;
        return Err(WorkerError::missing_process_id());
    };
    let mut failure = None;
    let loop_result = (|| -> Result<(), WorkerError> {
        *lock(&status, "status")? = Status::running();
        let _ = notifications.try_send(());
        let mut parser: Processor = Processor::new();
        let mut buffer = [0u8; 16384];
        let mut pending = VecDeque::new();
        let mut offset = 0;
        let mut exit_seen = None;
        loop {
            if cancelled.load(Ordering::Acquire) {
                break;
            }
            if terminal.is_poisoned() {
                return Err(LockError::new("screen").into());
            }
            if status.is_poisoned() {
                return Err(LockError::new("status").into());
            }
            if poisoned.load(Ordering::Acquire) {
                failure = Some(WorkerError::listener_lock_poisoned());
                break;
            }
            if overflow.load(Ordering::Acquire) {
                failure = Some(WorkerError::response_queue_overflow());
                break;
            }
            if pending.len() < 128 {
                for _ in 0..(128 - pending.len()) {
                    match receiver.try_recv() {
                        Ok(Command::Write(bytes)) => pending.push_back(bytes),
                        Ok(Command::Resize(dimensions, width, height)) => {
                            let result = pair
                                .master
                                .resize(PtySize {
                                    rows: dimensions.lines as u16,
                                    cols: dimensions.columns as u16,
                                    pixel_width: (dimensions.columns * width as usize)
                                        .min(u16::MAX as usize)
                                        as u16,
                                    pixel_height: (dimensions.lines * height as usize)
                                        .min(u16::MAX as usize)
                                        as u16,
                                })
                                .map_err(|source| WorkerError::resize_pty(source.into()));
                            if let Err(error) = result {
                                failure = Some(error);
                                break;
                            }
                            lock(&terminal, "screen")?.resize(dimensions);
                            *lock(&size, "size")? = WindowSize {
                                num_lines: dimensions.lines as u16,
                                num_cols: dimensions.columns as u16,
                                cell_width: width,
                                cell_height: height,
                            };
                            let _ = notifications.try_send(());
                        },
                        Err(_) => break,
                    }
                }
            }
            if failure.is_some() {
                break;
            }
            {
                let mut queue = lock(&responses, "response")?;
                while pending.len() < 128 {
                    let Some(bytes) = queue.pop_front() else {
                        break;
                    };
                    pending.push_back(bytes);
                }
            }
            for _ in 0..128 {
                let Some(bytes) = pending.front() else {
                    break;
                };
                match writer.write(&bytes[offset..]) {
                    Ok(0) => {
                        failure = Some(WorkerError::write_zero());
                        break;
                    },
                    Ok(count) => {
                        offset += count;
                        if offset == bytes.len() {
                            pending.pop_front();
                            offset = 0;
                        }
                    },
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) =>
                    {
                        break;
                    },
                    Err(error) => {
                        failure = Some(WorkerError::write_input(error));
                        break;
                    },
                }
            }
            let mut eof = false;
            for _ in 0..16 {
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        eof = true;
                        break;
                    },
                    Ok(count) => {
                        parser.advance(&mut *lock(&terminal, "screen")?, &buffer[..count]);
                        let _ = notifications.try_send(());
                    },
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) =>
                    {
                        break;
                    },
                    Err(error) if error.raw_os_error() == Some(libc::EIO) => {
                        eof = true;
                        break;
                    },
                    Err(error) => {
                        failure = Some(WorkerError::read_output(error));
                        break;
                    },
                }
            }
            if failure.is_some() {
                break;
            }
            if parser
                .sync_timeout()
                .sync_timeout()
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                parser.stop_sync(&mut *lock(&terminal, "screen")?);
                let _ = notifications.try_send(());
            }
            match child_exited(pid) {
                Ok(true) => {
                    let deadline = exit_seen
                        .get_or_insert_with(|| Instant::now() + Duration::from_millis(100));
                    if eof || Instant::now() >= *deadline {
                        break;
                    }
                },
                Ok(false) => {},
                Err(error) => {
                    failure = Some(WorkerError::check_exit(error));
                    break;
                },
            }
            if eof {
                thread::sleep(Duration::from_millis(10));
            }
            thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    })();
    if let Err(error) = loop_result {
        failure = Some(error);
    }
    let deadline = Instant::now() + Duration::from_millis(50);
    let session = pid as i32;
    let mut processes = session_processes(session, deadline);
    for group in [pair.master.process_group_leader(), Some(session)].into_iter().flatten() {
        if unsafe { libc::getsid(group) } == session {
            processes.push((group, group));
        }
    }
    for &(member, group) in &processes {
        signal_group(member, group, session, libc::SIGTERM);
    }
    loop {
        processes.retain(|&(member, group)| {
            (unsafe { libc::getsid(member) == session && libc::getpgid(member) == group })
                && process_alive(member)
        });
        if processes.is_empty() || Instant::now() >= deadline {
            break;
        }
        thread::sleep(
            Duration::from_millis(2).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
    for &(member, group) in &processes {
        signal_group(member, group, session, libc::SIGKILL);
    }
    drop(reader);
    drop(writer);
    drop(pair.master);
    let exit = child.wait().map_err(WorkerError::wait_shell)?;
    if let Some(error) = failure {
        return Err(error);
    }
    *lock(&status, "status")? = Status::exited(exit.exit_code(), exit.signal().map(str::to_owned));
    Ok(())
}
