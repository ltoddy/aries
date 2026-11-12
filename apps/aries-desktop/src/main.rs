mod acp;
mod error;
mod terminal;
mod workspace;

use gpui_kit::assets::AllAssets;
use gpui_kit::base::Root;
use gpui_kit::component::{ActiveTheme, TitleBar};
use gpui_kit::*;

use crate::acp::aries_config;
use crate::workspace::WorkspaceView;
use crate::workspace::settings::Preferences;

gpui_kit::actions!(
    orbit,
    [
        Quit,
        OpenSettings,
        NewSession,
        CloseTab,
        PreviousTab,
        NextTab,
        PreviousUserMessage,
        NextUserMessage,
        ToggleTerminal,
        ToggleSidebar,
    ]
);

#[derive(Debug, thiserror::Error)]
enum StartupError {
    #[error("无法读取启动目录")]
    Directory { source: std::io::Error },
    #[error("无法访问启动目录 {}", path.display())]
    Access { path: std::path::PathBuf, source: std::io::Error },
}

impl StartupError {
    fn directory(source: std::io::Error) -> Self {
        Self::Directory { source }
    }

    fn access(path: std::path::PathBuf, source: std::io::Error) -> Self {
        Self::Access { path, source }
    }
}

fn main() -> std::process::ExitCode {
    error::init();
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            error::log("无法启动 Aries Desktop", &error);
            std::process::ExitCode::FAILURE
        },
    }
}

fn run() -> Result<(), StartupError> {
    let directory = std::env::current_dir().map_err(StartupError::directory)?;
    let cwd = directory.canonicalize().map_err(|source| StartupError::access(directory, source))?;
    gpui_kit::application().with_assets(AllAssets).run(move |cx| {
        gpui_kit::init(cx);
        crate::terminal::init(cx);
        cx.bind_keys([
            KeyBinding::new(
                if cfg!(target_os = "macos") { "cmd-f" } else { "ctrl-f" },
                gpui_kit::component::input::Search,
                Some("GitDiff"),
            ),
            KeyBinding::new("escape", gpui_kit::component::input::Escape, Some("GitDiff")),
        ]);
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None), KeyBinding::new("ctrl-q", Quit, None)]);
        #[cfg(target_os = "macos")]
        cx.bind_keys([
            KeyBinding::new("cmd-,", OpenSettings, None),
            KeyBinding::new("cmd-n", NewSession, None),
            KeyBinding::new("cmd-f", ToggleTerminal, None),
            KeyBinding::new("cmd-b", ToggleSidebar, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        #[cfg(target_os = "macos")]
        cx.bind_keys([
            KeyBinding::new("cmd-w", CloseTab, None),
            KeyBinding::new("cmd-shift-[", PreviousTab, None),
            KeyBinding::new("cmd-shift-]", NextTab, None),
            KeyBinding::new("cmd-alt-up", PreviousUserMessage, None),
            KeyBinding::new("cmd-alt-down", NextUserMessage, None),
        ]);
        #[cfg(not(target_os = "macos"))]
        cx.bind_keys([
            KeyBinding::new("ctrl-w", CloseTab, None),
            KeyBinding::new("ctrl-b", ToggleSidebar, None),
            KeyBinding::new("ctrl-shift-[", PreviousTab, None),
            KeyBinding::new("ctrl-shift-]", NextTab, None),
            KeyBinding::new("ctrl-alt-up", PreviousUserMessage, None),
            KeyBinding::new("ctrl-alt-down", NextUserMessage, None),
        ]);
        cx.bind_keys([
            KeyBinding::new("ctrl-tab", NextTab, None),
            KeyBinding::new("ctrl-shift-tab", PreviousTab, None),
        ]);
        #[cfg(target_os = "macos")]
        cx.set_menus([Menu::new("Aries Desktop").items([
            MenuItem::action("设置…", OpenSettings),
            MenuItem::separator(),
            MenuItem::action("退出 Aries Desktop", Quit),
        ])]);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1180.), px(760.)),
                cx,
            ))),
            window_min_size: Some(size(px(760.), px(520.))),
            window_background: if cfg!(target_os = "macos") {
                WindowBackgroundAppearance::Blurred
            } else {
                WindowBackgroundAppearance::Opaque
            },
            titlebar: Some(TitlebarOptions {
                title: Some("Aries Desktop".into()),
                ..TitleBar::title_bar_options()
            }),
            ..TitleBar::window_options()
        };
        match gpui_kit::open_window(options, cx, |window, cx| {
            window.on_window_should_close(cx, |_, cx| {
                cx.quit();
                false
            });
            cx.new(|cx| WorkspaceView::new(cwd, Preferences::path(), aries_config(), window, cx))
        }) {
            Ok((handle, _)) => {
                if let Err(error) = cx.update_window(handle, |_, window, cx| {
                    Root::update(window, cx, |root, _, cx| {
                        root.style().background = Some(cx.theme().background.opacity(0.).into());
                        cx.notify();
                    });
                }) {
                    error::log("无法配置窗口外观", error.as_ref());
                }
            },
            Err(error) => {
                error::log("无法创建窗口", error.as_ref());
                cx.quit();
            },
        }
        cx.activate(true);
    });
    Ok(())
}
