mod acp;
mod error;
pub mod ipc;
mod keymap;
mod startup;
mod terminal;
mod workspace;

gpui_kit::actions!(
    aries_desktop,
    [
        Quit,
        Hide,
        OpenSettings,
        NewSession,
        CloseTab,
        PreviousTab,
        NextTab,
        PreviousUserMessage,
        NextUserMessage,
        ToggleTerminal,
        ToggleSidebar,
        ShowSessionsSidebar,
        ShowFilesSidebar,
        ShowGitSidebar,
    ]
);

impl ShowSessionsSidebar {
    pub fn new() -> Self {
        Self
    }
}

impl ShowFilesSidebar {
    pub fn new() -> Self {
        Self
    }
}

impl ShowGitSidebar {
    pub fn new() -> Self {
        Self
    }
}

fn main() -> std::process::ExitCode {
    error::init();
    match startup::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            error::log("无法启动 Aries Desktop", &error);
            std::process::ExitCode::FAILURE
        },
    }
}
