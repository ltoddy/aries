use crate::terminal::session::error::{ResizeError, StartError, WriteError};
use crate::terminal::session::worker::{Worker, run};
use crate::terminal::session::*;

#[cfg_attr(not(unix), allow(dead_code))]
pub enum Command {
    Write(Vec<u8>),
    Resize(Dimensions, u16, u16),
}

impl Command {
    pub fn write(bytes: Vec<u8>) -> Self {
        Self::Write(bytes)
    }
    pub fn resize(dimensions: Dimensions, width: u16, height: u16) -> Self {
        Self::Resize(dimensions, width, height)
    }
}

#[derive(Clone)]
pub struct Listener {
    pub responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    pub size: Arc<Mutex<WindowSize>>,
    pub overflow: Arc<AtomicBool>,
    pub poisoned: Arc<AtomicBool>,
}

impl Listener {
    pub fn new(
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
