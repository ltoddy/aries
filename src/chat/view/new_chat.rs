use std::path::{Path, PathBuf};

use gpui_kit::component::WindowExt;
use gpui_kit::component::notification::Notification;
use gpui_kit::*;

use crate::acp::{Command, Event};
use crate::chat::view::ChatView;

#[derive(Clone, Copy, Debug)]
pub enum NewChatAction {
    Create,
    SendPrompt,
}

impl ChatView {
    pub fn can_start_new_chat(&self) -> bool {
        self.is_session_idle()
    }

    pub fn choose_session_directory(
        &mut self,
        action: NewChatAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_start_new_chat() {
            return;
        }
        self.choosing_directory = true;
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("选择新会话的项目目录".into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            let result: anyhow::Result<Option<PathBuf>> = async {
                let Some(path) = selection.await??.and_then(|paths| paths.into_iter().next())
                else {
                    return Ok(None);
                };
                let directory = cx
                    .background_executor()
                    .spawn(async move { validate_directory(&path) })
                    .await?;
                Ok(Some(directory))
            }
            .await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.choosing_directory = false;
                match result {
                    Ok(Some(directory)) => {
                        if matches!(action, NewChatAction::Create)
                            && let Some(host) = &this.tab_host
                        {
                            let _ = host.update(cx, |host, cx| {
                                host.new_tab_in_directory(directory, window, cx);
                            });
                        } else {
                            this.start_new_chat(directory, action, window, cx);
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        window.push_notification(
                            Notification::error(format!("无法选择项目目录：{error}")),
                            cx,
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn new_chat_in_directory(
        &mut self,
        directory: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_start_new_chat() {
            return;
        }
        self.choosing_directory = true;
        cx.spawn_in(window, async move |view, cx| {
            let (result, directory) = cx
                .background_executor()
                .spawn(async move { (validate_directory(&directory), directory) })
                .await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.choosing_directory = false;
                match result {
                    Ok(directory) => {
                        this.session_list.expand_project(&directory);
                        if let Some(host) = &this.tab_host {
                            let _ = host.update(cx, |host, cx| {
                                host.new_tab_in_directory(directory, window, cx);
                            });
                        } else {
                            this.start_new_chat(directory, NewChatAction::Create, window, cx);
                        }
                    }
                    Err(error) => window.push_notification(
                        Notification::error(format!(
                            "无法在目录 {} 新建会话：{error}。请检查目录是否存在且可访问。",
                            directory.display()
                        )),
                        cx,
                    ),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn start_new_chat(
        &mut self,
        directory: PathBuf,
        action: NewChatAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_start_new_chat() {
            return;
        }
        self.dismiss_permissions();
        self.save_draft(cx);
        if self.disconnected {
            self.receiver = None;
            self.bridge = None;
        }
        self.reset_chat();
        self.cwd = directory;
        self.watch_git_info(cx);
        self.session_directory_selected = true;
        self.disconnected = false;
        self.input.update(cx, |input, cx| {
            if matches!(action, NewChatAction::Create) {
                input.set_value("", window, cx);
            }
            input.focus(window, cx);
        });
        self.connect(window, cx);
        self.creating_session = Some(action);
        self.status = "正在创建会话…".into();
        if !self
            .bridge
            .as_ref()
            .is_some_and(|bridge| bridge.send(Command::NewSession(self.cwd.clone())).is_ok())
        {
            self.apply_event(
                Event::Disconnected("无法创建会话，请重新连接后重试。".into()),
                window,
                cx,
            );
        }
        cx.notify();
    }
}

fn validate_directory(path: &Path) -> std::io::Result<PathBuf> {
    let directory = path.canonicalize()?;
    if !directory.is_dir() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotADirectory, "请选择文件夹"));
    }
    Ok(directory)
}
