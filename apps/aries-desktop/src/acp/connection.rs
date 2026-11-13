use std::collections::VecDeque;
use std::ops::ControlFlow;
use std::sync::Arc;

use agent_client_protocol::v1::ConnectionTo;
use agent_client_protocol::v1::schema::{AgentCapabilities, SessionArchiveFilter};
use agent_client_protocol::{AcpAgent, AcpAgentConfig, Agent, Error};
use async_channel::{Receiver, Sender};
use tokio::sync::watch;

use crate::acp::handlers::{
    create_terminal, kill_terminal, release_terminal, request_permission, session_update,
    terminal_output, wait_for_terminal_exit,
};
use crate::acp::requests::{
    delete_sessions, initialize, list_sessions, load_session, new_session, prompt,
    session_management, set_session_config_option,
};
use crate::acp::terminal::Terminals;
use crate::acp::{Command, DeletionReport, Event};

pub struct AgentConnection {
    client: ConnectionTo<Agent>,
    commands: Receiver<Command>,
    events: Sender<Event>,
    cancel: watch::Sender<bool>,
    capabilities: AgentCapabilities,
}

impl std::fmt::Debug for AgentConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentConnection")
            .field("capabilities", &self.capabilities)
            .finish_non_exhaustive()
    }
}

impl AgentConnection {
    fn new(
        client: ConnectionTo<Agent>,
        commands: Receiver<Command>,
        events: Sender<Event>,
        cancel: watch::Sender<bool>,
        capabilities: AgentCapabilities,
    ) -> Self {
        Self { client, commands, events, cancel, capabilities }
    }

    pub fn client(&self) -> &ConnectionTo<Agent> {
        &self.client
    }

    pub fn commands(&self) -> &Receiver<Command> {
        &self.commands
    }

    pub fn set_cancelled(&self, cancelled: bool) {
        self.cancel.send_replace(cancelled);
    }

    pub async fn emit(&self, event: Event) {
        let _ = self.events.send(event).await;
    }

    pub async fn request_sessions(&self, archived: Option<SessionArchiveFilter>) {
        let result =
            if archived.is_some() && self.capabilities.session_capabilities.archive.is_none() {
                Err(Error::internal_error().data("Agent 不支持会话归档筛选"))
            } else if self.capabilities.session_capabilities.list.is_some() {
                list_sessions::request(&self.client, &self.events, archived)
            } else {
                Err(Error::internal_error().data("Agent 不支持会话列表"))
            };
        if let Err(error) = result {
            let _ = self.events.send(Event::Sessions(Err(error))).await;
        }
    }

    pub async fn handle_busy_command(&self, command: Command) {
        match command {
            Command::ListSessions(archived) => self.request_sessions(archived).await,
            Command::DeleteSessions(ids) => {
                let _ = self
                    .events
                    .send(Event::SessionsDeleted(DeletionReport::rejected(
                        ids,
                        "请等待当前回复结束后再删除会话",
                    )))
                    .await;
            },
            Command::LoadSession(request) => {
                let _ = self
                    .events
                    .send(Event::SessionLoaded {
                        session_id: request.session_id,
                        result: Err(Error::internal_error().data("请等待当前回复结束后再加载会话")),
                    })
                    .await;
            },
            Command::SetSessionConfigOption(request) => {
                self.emit(Event::SessionConfigSet {
                    session_id: request.session_id,
                    config_id: request.config_id,
                    result: Err(Error::internal_error().data("请等待当前操作结束后再修改配置")),
                })
                .await;
            },
            Command::SetSessionTitle(request) => {
                self.emit(Event::session_title_set(
                    request.session_id,
                    Err(Error::internal_error().data("请等待当前操作结束后再重命名会话")),
                ))
                .await;
            },
            Command::ArchiveSession(session_id) => {
                self.emit(Event::session_archived(
                    session_id,
                    true,
                    Err(Error::internal_error().data("请等待当前操作结束后再归档会话")),
                ))
                .await;
            },
            Command::UnarchiveSession(session_id) => {
                self.emit(Event::session_archived(
                    session_id,
                    false,
                    Err(Error::internal_error().data("请等待当前操作结束后再恢复会话")),
                ))
                .await;
            },
            Command::ForkSession(request) => {
                self.emit(Event::session_forked(
                    request.session_id,
                    Err(Error::internal_error().data("请等待当前操作结束后再复制会话")),
                ))
                .await;
            },
            Command::NewSession(_) | Command::Prompt { .. } | Command::Cancel => {},
        }
    }

    async fn run(&self, mut pending: VecDeque<Command>) -> Result<(), Error> {
        loop {
            let command = match pending.pop_front() {
                Some(command) => command,
                None => tokio::select! {
                    command = self.commands.recv() => match command {
                        Ok(command) => command,
                        Err(_) => break,
                    },
                    _ = self.client.incoming_closed() => return Err(closed()),
                },
            };
            match command {
                Command::NewSession(cwd) => {
                    self.set_cancelled(false);
                    let Some(response) = new_session::request(self, cwd).await? else {
                        break;
                    };
                    self.emit(Event::Ready(Box::new(response))).await;
                },
                Command::ListSessions(archived) => self.request_sessions(archived).await,
                Command::DeleteSessions(ids) => {
                    let result = delete_sessions::request(&self.client, ids).await;
                    let _ = self.events.send(Event::SessionsDeleted(result)).await;
                },
                Command::LoadSession(request) => {
                    let session_id = request.session_id.clone();
                    self.cancel.send_replace(false);
                    let result = if self.capabilities.load_session {
                        load_session::request(&self.client, *request).await.map_err(Error::from)
                    } else {
                        Err(Error::internal_error().data("Agent 不支持加载会话"))
                    };
                    let _ = self.events.send(Event::SessionLoaded { session_id, result }).await;
                },
                Command::Prompt { session_id, content } => {
                    if prompt::request(self, session_id, content).await? == ControlFlow::Break(()) {
                        break;
                    }
                },
                Command::SetSessionConfigOption(request) => {
                    let session_id = request.session_id.clone();
                    let config_id = request.config_id.clone();
                    let result = set_session_config_option::request(&self.client, *request)
                        .await
                        .map_err(Error::from);
                    self.emit(Event::SessionConfigSet { session_id, config_id, result }).await;
                },
                Command::SetSessionTitle(request) => {
                    let session_id = request.session_id.clone();
                    let result = if self.capabilities.session_capabilities.title.is_some() {
                        session_management::set_title(&self.client, *request)
                            .await
                            .map_err(Error::from)
                    } else {
                        Err(Error::internal_error().data("Agent 不支持重命名会话"))
                    };
                    self.emit(Event::session_title_set(session_id, result)).await;
                },
                Command::ArchiveSession(session_id) => {
                    let result = if self.capabilities.session_capabilities.archive.is_some() {
                        session_management::archive(&self.client, session_id.clone())
                            .await
                            .map_err(Error::from)
                    } else {
                        Err(Error::internal_error().data("Agent 不支持归档会话"))
                    };
                    self.emit(Event::session_archived(session_id, true, result)).await;
                },
                Command::UnarchiveSession(session_id) => {
                    let result = if self.capabilities.session_capabilities.archive.is_some() {
                        session_management::unarchive(&self.client, session_id.clone())
                            .await
                            .map_err(Error::from)
                    } else {
                        Err(Error::internal_error().data("Agent 不支持恢复会话"))
                    };
                    self.emit(Event::session_archived(session_id, false, result)).await;
                },
                Command::ForkSession(request) => {
                    let session_id = request.session_id.clone();
                    let result = if self.capabilities.session_capabilities.fork.is_some() {
                        session_management::fork(&self.client, *request).await.map_err(Error::from)
                    } else {
                        Err(Error::internal_error().data("Agent 不支持复制会话"))
                    };
                    self.emit(Event::session_forked(session_id, result)).await;
                },
                Command::Cancel => {},
            }
        }
        Ok(())
    }
}

pub fn closed() -> Error {
    Error::internal_error().data("Agent 已关闭连接")
}

pub async fn connect(
    config: AcpAgentConfig,
    commands: Receiver<Command>,
    events: Sender<Event>,
) -> Result<(), Error> {
    let (cancel, cancel_rx) = watch::channel(false);
    let terminals = Arc::new(Terminals::new());
    let create = terminals.clone();
    let output = terminals.clone();
    let release = terminals.clone();
    let kill = terminals.clone();
    let wait = terminals;
    let updates = events.clone();
    let permissions = events.clone();
    agent_client_protocol::Client.v1()
        .name("aries-desktop")
        .on_receive_notification(
            async move |notification, _connection| {
                session_update::handle(notification, &updates).await
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .on_receive_request(
            async move |request, responder, connection| {
                request_permission::handle(request, responder, connection, &permissions, cancel_rx.clone()).await
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request, responder, _connection| {
                create_terminal::handle(request, responder, &create).await
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request, responder, _connection| {
                terminal_output::handle(request, responder, &output).await
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request, responder, connection| {
                release_terminal::handle(request, responder, connection, release.clone()).await
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request, responder, connection| {
                kill_terminal::handle(request, responder, connection, kill.clone()).await
            },
            agent_client_protocol::on_receive_request!(),
        )
        .on_receive_request(
            async move |request, responder, connection| {
                wait_for_terminal_exit::handle(request, responder, connection, wait.clone()).await
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_with(AcpAgent::new(config), |client| async move {
            let mut pending = VecDeque::new();
            let capabilities = {
                let setup = initialize::request(&client);
                tokio::pin!(setup);
                loop {
                    tokio::select! {
                        biased;
                        command = commands.recv() => match command {
                            Ok(Command::Cancel) => return Err(Error::internal_error().data("已停止连接 Aries")),
                            Ok(command) => pending.push_back(command),
                            Err(_) => return Ok(()),
                        },
                        _ = client.incoming_closed() => return Err(closed()),
                        result = &mut setup => break result.map_err(Error::from)?,
                    }
                }
            };
            let _ = events.send(Event::connected(
                capabilities.session_capabilities.list.is_some(),
                capabilities.load_session,
                capabilities.prompt_capabilities.clone(),
                capabilities.session_capabilities.clone(),
            )).await;
            AgentConnection::new(client, commands, events, cancel, capabilities)
                .run(pending).await
        })
        .await
}
