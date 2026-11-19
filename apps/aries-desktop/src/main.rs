mod acp;
mod error;
mod keymap;
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
        SwitchSidebar,
    ]
);

impl SwitchSidebar {
    fn new() -> Self {
        Self
    }
}

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
        crate::keymap::init(cx);
        #[cfg(target_os = "macos")]
        cx.on_action(|_: &Hide, cx| cx.hide());
        #[cfg(target_os = "macos")]
        cx.set_menus([Menu::new("Aries Desktop").items([
            MenuItem::action("设置…", OpenSettings),
            MenuItem::action("切换会话 / 文件", SwitchSidebar::new()),
            MenuItem::separator(),
            MenuItem::action("隐藏 Aries Desktop", Hide),
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
            cx.new(|cx| WorkspaceView::new(cwd, Preferences::path(), aries_config(), window, cx))
        }) {
            Ok((handle, workspace)) => {
                let quit_workspace = workspace.downgrade();
                cx.on_action(move |_: &Quit, cx| {
                    cx.stop_propagation();
                    let workspace = quit_workspace.clone();
                    cx.defer(move |cx| {
                        if let Some(workspace) = workspace.upgrade() {
                            if let Err(error) = handle.update(cx, |_, window, cx| {
                                workspace
                                    .update(cx, |workspace, cx| workspace.request_quit(window, cx));
                            }) {
                                error::log("无法请求退出", error.as_ref());
                            }
                        } else {
                            cx.quit();
                        }
                    });
                });
                let close_workspace = workspace.downgrade();
                if let Err(error) = cx.update_window(handle, |_, window, cx| {
                    window.on_window_should_close(cx, move |window, cx| {
                        let workspace = close_workspace.clone();
                        window.defer(cx, move |window, cx| {
                            let _ = workspace
                                .update(cx, |workspace, cx| workspace.request_quit(window, cx));
                        });
                        false
                    });
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
