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
use anyhow::Context as _;
#[cfg(unix)]
use portable_pty::{CommandBuilder, PtySize, native_pty_system};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Starting,
    Running,
    Exited { code: u32, signal: Option<String> },
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dimensions {
    pub columns: usize,
    pub lines: usize,
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

#[derive(Debug, thiserror::Error)]
pub enum StartError {
    #[error(transparent)]
    InvalidDimensions(#[from] InvalidDimensions),
    #[error("启动终端工作线程失败：{source}")]
    WorkerSpawn {
        #[source]
        source: std::io::Error,
    },
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
}

#[cfg_attr(not(unix), allow(dead_code))]
enum Command {
    Write(Vec<u8>),
    Resize(Dimensions, u16, u16),
}

pub struct Listener {
    responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    size: Arc<Mutex<WindowSize>>,
    overflow: Arc<AtomicBool>,
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        let response = match event {
            Event::PtyWrite(text) => Some(text),
            Event::ColorRequest(index, format) if index < 256 => {
                Some(format(crate::terminal::paint::indexed_rgb(index)))
            },
            Event::TextAreaSizeRequest(format) => {
                Some(format(*self.size.lock().expect("terminal size lock poisoned")))
            },
            _ => None,
        };
        if let Some(response) = response {
            let mut queue = self.responses.lock().expect("terminal response lock poisoned");
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
        let size = Arc::new(Mutex::new(WindowSize {
            num_lines: dimensions.lines as u16,
            num_cols: dimensions.columns as u16,
            cell_width: 0,
            cell_height: 0,
        }));
        let terminal = Arc::new(Mutex::new(Term::new(
            Config { scrolling_history: 10000, osc52: Osc52::Disabled, ..Config::default() },
            &dimensions,
            Listener {
                responses: responses.clone(),
                size: size.clone(),
                overflow: overflow.clone(),
            },
        )));
        let status = Arc::new(Mutex::new(Status::Starting));
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
                    Worker {
                        terminal,
                        status: status.clone(),
                        receiver,
                        cancelled,
                        responses,
                        size,
                        overflow,
                        notifications: notifications.clone(),
                    },
                );
                if let Err(error) = result {
                    *status.lock().expect("terminal status lock poisoned") =
                        Status::Failed(format!("{error:#}"));
                }
                let _ = notifications.try_send(());
                let _ = completed.try_send(());
            })
            .map_err(|source| StartError::WorkerSpawn { source })?;
        Ok(session)
    }

    pub async fn shutdown(self) {
        self.cancelled.store(true, Ordering::Release);
        let _ = self.completion.recv().await;
    }

    pub fn terminal(&self) -> MutexGuard<'_, Term<Listener>> {
        self.terminal.lock().expect("terminal screen lock poisoned")
    }

    pub fn status(&self) -> Status {
        self.status.lock().expect("terminal status lock poisoned").clone()
    }

    pub fn write(&self, bytes: Vec<u8>) -> Result<(), WriteError> {
        if bytes.is_empty() {
            return Ok(());
        }
        if bytes.len() > 65536 {
            return Err(WriteError::TooLarge { size: bytes.len() });
        }
        if !matches!(
            *self.status.lock().expect("terminal status lock poisoned"),
            Status::Starting | Status::Running
        ) {
            return Err(WriteError::NotRunning);
        }
        self.commands.try_send(Command::Write(bytes)).map_err(|error| match error {
            mpsc::TrySendError::Full(_) => WriteError::QueueFull,
            mpsc::TrySendError::Disconnected(_) => WriteError::QueueClosed,
        })
    }

    pub fn resize(
        &self,
        dimensions: Dimensions,
        cell_width: u16,
        cell_height: u16,
    ) -> Result<(), ResizeError> {
        validate_dimensions(dimensions)?;
        self.commands.try_send(Command::Resize(dimensions, cell_width, cell_height)).map_err(
            |error| match error {
                mpsc::TrySendError::Full(_) => ResizeError::QueueFull,
                mpsc::TrySendError::Disconnected(_) => ResizeError::QueueClosed,
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
        return Err(InvalidDimensions { dimensions });
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
    responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    size: Arc<Mutex<WindowSize>>,
    overflow: Arc<AtomicBool>,
    notifications: async_channel::Sender<()>,
}

#[cfg(not(unix))]
fn run(_: PathBuf, _: Dimensions, _: Worker) -> anyhow::Result<()> {
    Err(WorkerError::UnsupportedPlatform.into())
}

#[cfg(unix)]
fn run(directory: PathBuf, dimensions: Dimensions, worker: Worker) -> anyhow::Result<()> {
    let Worker { terminal, status, receiver, cancelled, responses, size, overflow, notifications } =
        worker;
    if cancelled.load(Ordering::Acquire) {
        *status.lock().expect("terminal status lock poisoned") =
            Status::Exited { code: 0, signal: None };
        return Ok(());
    }
    let directory = std::fs::canonicalize(directory).context("resolving terminal directory")?;
    if !directory.is_dir() {
        return Err(WorkerError::NotDirectory(directory).into());
    }
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: dimensions.lines as u16,
            cols: dimensions.columns as u16,
            ..PtySize::default()
        })
        .context("opening PTY")?;
    let mut reader = pair.master.try_clone_reader().context("cloning PTY reader")?;
    let mut writer = pair.master.take_writer().context("taking PTY writer")?;
    #[cfg(unix)]
    {
        let fd = pair.master.as_raw_fd().ok_or(WorkerError::MissingFileDescriptor)?;
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(std::io::Error::last_os_error()).context("setting PTY nonblocking mode");
        }
    }
    let mut command = CommandBuilder::new(shell());
    command.arg("-l");
    command.cwd(directory);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    if cancelled.load(Ordering::Acquire) {
        *status.lock().expect("terminal status lock poisoned") =
            Status::Exited { code: 0, signal: None };
        return Ok(());
    }
    let mut child = pair.slave.spawn_command(command).context("starting user shell")?;
    drop(pair.slave);
    let Some(pid) = child.process_id() else {
        let _ = child.kill();
        child.wait().context("waiting for shell without process identifier")?;
        return Err(WorkerError::MissingProcessId.into());
    };
    *status.lock().expect("terminal status lock poisoned") = Status::Running;
    let _ = notifications.try_send(());
    let mut parser: Processor = Processor::new();
    let mut buffer = [0u8; 16384];
    let mut pending = VecDeque::new();
    let mut offset = 0;
    let mut failure = None;
    let mut exit_seen = None;
    loop {
        if cancelled.load(Ordering::Acquire) {
            break;
        }
        if overflow.load(Ordering::Acquire) {
            failure = Some(WorkerError::ResponseQueueOverflow.into());
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
                            .context("resizing PTY");
                        if let Err(error) = result {
                            failure = Some(error);
                            break;
                        }
                        terminal.lock().expect("terminal screen lock poisoned").resize(dimensions);
                        *size.lock().expect("terminal size lock poisoned") = WindowSize {
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
            let mut queue = responses.lock().expect("terminal response lock poisoned");
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
                    failure = Some(WorkerError::WriteZero.into());
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
                    failure = Some(anyhow::Error::new(error).context("writing PTY input"));
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
                    parser.advance(
                        &mut *terminal.lock().expect("terminal screen lock poisoned"),
                        &buffer[..count],
                    );
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
                    failure = Some(anyhow::Error::new(error).context("reading PTY output"));
                    break;
                },
            }
        }
        if failure.is_some() {
            break;
        }
        if parser.sync_timeout().sync_timeout().is_some_and(|deadline| Instant::now() >= deadline) {
            parser.stop_sync(&mut *terminal.lock().expect("terminal screen lock poisoned"));
            let _ = notifications.try_send(());
        }
        match child_exited(pid) {
            Ok(true) => {
                let deadline =
                    exit_seen.get_or_insert_with(|| Instant::now() + Duration::from_millis(100));
                if eof || Instant::now() >= *deadline {
                    break;
                }
            },
            Ok(false) => {},
            Err(error) => {
                failure = Some(anyhow::Error::new(error).context("checking shell exit status"));
                break;
            },
        }
        if eof {
            thread::sleep(Duration::from_millis(10));
        }
        thread::sleep(Duration::from_millis(5));
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
    let exit = child.wait().context("waiting for shell")?;
    if let Some(error) = failure {
        return Err(error);
    }
    *status.lock().expect("terminal status lock poisoned") =
        Status::Exited { code: exit.exit_code(), signal: exit.signal().map(str::to_owned) };
    Ok(())
}
