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
use anyhow::{Context as _, Result, bail};
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
            }
            Event::TextAreaSizeRequest(format) => {
                Some(format(*self.size.lock().expect("terminal size lock poisoned")))
            }
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
        directory: &Path,
        dimensions: Dimensions,
        notifications: async_channel::Sender<()>,
    ) -> Result<Self> {
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
        let directory = directory.to_path_buf();
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
            .context("starting terminal worker")?;
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

    pub fn write(&self, bytes: Vec<u8>) -> Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }
        if bytes.len() > 65536 {
            bail!("终端输入不能超过 64 KiB");
        }
        if !matches!(self.status(), Status::Starting | Status::Running) {
            bail!("终端未运行");
        }
        self.commands.try_send(Command::Write(bytes)).context("终端输入队列已满或已关闭")
    }

    pub fn resize(&self, dimensions: Dimensions, cell_width: u16, cell_height: u16) -> Result<()> {
        validate_dimensions(dimensions)?;
        self.commands
            .try_send(Command::Resize(dimensions, cell_width, cell_height))
            .context("终端尺寸调整队列已满或已关闭")
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

fn validate_dimensions(dimensions: Dimensions) -> Result<()> {
    if !(2..=1000).contains(&dimensions.columns) || !(1..=1000).contains(&dimensions.lines) {
        bail!("终端尺寸超出允许范围");
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
fn run(_: PathBuf, _: Dimensions, _: Worker) -> Result<()> {
    bail!("terminal PTY worker currently requires Unix")
}

#[cfg(unix)]
fn run(directory: PathBuf, dimensions: Dimensions, worker: Worker) -> Result<()> {
    let Worker { terminal, status, receiver, cancelled, responses, size, overflow, notifications } =
        worker;
    if cancelled.load(Ordering::Acquire) {
        *status.lock().expect("terminal status lock poisoned") =
            Status::Exited { code: 0, signal: None };
        return Ok(());
    }
    let directory = std::fs::canonicalize(directory).context("resolving terminal directory")?;
    if !directory.is_dir() {
        bail!("terminal directory is not a directory");
    }
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: dimensions.lines as u16,
            cols: dimensions.columns as u16,
            ..PtySize::default()
        })
        .context("opening PTY")?;
    let mut reader = pair.master.try_clone_reader()?;
    let mut writer = pair.master.take_writer()?;
    #[cfg(unix)]
    {
        let fd = pair.master.as_raw_fd().context("PTY has no file descriptor")?;
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(std::io::Error::last_os_error().into());
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
        bail!("shell has no process identifier");
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
            failure = Some(anyhow::anyhow!("terminal response queue overflow"));
            break;
        }
        if pending.len() < 128 {
            for _ in 0..(128 - pending.len()) {
                match receiver.try_recv() {
                    Ok(Command::Write(bytes)) => pending.push_back(bytes),
                    Ok(Command::Resize(dimensions, width, height)) => {
                        let result = pair.master.resize(PtySize {
                            rows: dimensions.lines as u16,
                            cols: dimensions.columns as u16,
                            pixel_width: (dimensions.columns * width as usize)
                                .min(u16::MAX as usize)
                                as u16,
                            pixel_height: (dimensions.lines * height as usize)
                                .min(u16::MAX as usize)
                                as u16,
                        });
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
                    }
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
                    failure = Some(anyhow::anyhow!("PTY write returned zero"));
                    break;
                }
                Ok(count) => {
                    offset += count;
                    if offset == bytes.len() {
                        pending.pop_front();
                        offset = 0;
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    break;
                }
                Err(error) => {
                    failure = Some(error.into());
                    break;
                }
            }
        }
        let mut eof = false;
        for _ in 0..16 {
            match reader.read(&mut buffer) {
                Ok(0) => {
                    eof = true;
                    break;
                }
                Ok(count) => {
                    parser.advance(
                        &mut *terminal.lock().expect("terminal screen lock poisoned"),
                        &buffer[..count],
                    );
                    let _ = notifications.try_send(());
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) =>
                {
                    break;
                }
                Err(error) if error.raw_os_error() == Some(libc::EIO) => {
                    eof = true;
                    break;
                }
                Err(error) => {
                    failure = Some(error.into());
                    break;
                }
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
            }
            Ok(false) => {}
            Err(error) => {
                failure = Some(error.into());
                break;
            }
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
