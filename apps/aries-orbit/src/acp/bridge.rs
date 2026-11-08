use agent_client_protocol::AcpAgentConfig;
use async_channel::{Receiver, Sender};
use tokio::sync::oneshot;

use crate::acp::{Command, Event, connection};

#[derive(Debug)]
pub struct Bridge {
    commands: Sender<Command>,
    events: Receiver<Event>,
    stop: Option<oneshot::Sender<()>>,
    done: Receiver<()>,
}

impl Bridge {
    pub fn start(config: AcpAgentConfig) -> std::io::Result<Self> {
        let (commands, command_rx) = async_channel::unbounded();
        let (events_tx, events) = async_channel::unbounded();
        let (stop, shutdown) = oneshot::channel();
        let (done_tx, done) = async_channel::bounded(1);
        std::thread::Builder::new().name("aries-acp".into()).spawn(move || {
            match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime.block_on(async {
                    tokio::select! {
                        _ = shutdown => {}
                        result = connection::connect(config, command_rx, events_tx.clone()) => {
                            let message = result.err().map(|e| e.to_string())
                                .unwrap_or_else(|| "Agent 连接已关闭".into());
                            let _ = events_tx.send(Event::Disconnected(message)).await;
                        }
                    }
                }),
                Err(error) => {
                    let _ = events_tx.try_send(Event::Disconnected(error.to_string()));
                }
            }
            let _ = done_tx.try_send(());
        })?;
        Ok(Self { commands, events, stop: Some(stop), done })
    }

    pub fn send(&self, command: Command) -> Result<(), async_channel::TrySendError<Command>> {
        self.commands.try_send(command)
    }

    pub fn events(&self) -> &Receiver<Event> {
        &self.events
    }

    pub async fn shutdown(mut self) {
        self.request_shutdown();
        let _ = self.done.recv().await;
    }

    fn request_shutdown(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.commands.close();
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.request_shutdown();
    }
}
