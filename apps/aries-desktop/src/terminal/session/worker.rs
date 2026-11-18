use crate::terminal::session::error::WorkerError;
use crate::terminal::session::runtime::{Command, Listener};
use crate::terminal::session::*;

#[cfg_attr(not(unix), allow(dead_code))]
pub struct Worker {
    terminal: Arc<Mutex<Term<Listener>>>,
    status: Arc<Mutex<Status>>,
    receiver: mpsc::Receiver<Command>,
    cancelled: Arc<AtomicBool>,
    listener: Listener,
    notifications: async_channel::Sender<()>,
}

impl Worker {
    pub fn new(
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
pub fn run(_: PathBuf, _: Dimensions, _: Worker) -> Result<(), WorkerError> {
    Err(WorkerError::unsupported_platform())
}

#[cfg(unix)]
pub fn run(directory: PathBuf, dimensions: Dimensions, worker: Worker) -> Result<(), WorkerError> {
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
