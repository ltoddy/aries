use std::path::PathBuf;

use aries_init::GlobalContext;
use gpui_kit::assets::AllAssets;
use gpui_kit::base::Root;
use gpui_kit::component::{ActiveTheme, TitleBar};
use gpui_kit::*;

use crate::acp::aries_config;
use crate::ipc::{Ipc, IpcError};
use crate::workspace::WorkspaceView;
use crate::workspace::settings::Preferences;
use crate::{OpenWorkspace, Quit, TogglePet, error, pet};

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("无法启动 Tokio 运行时")]
    Runtime { source: std::io::Error },
    #[error("无法访问启动目录 {}", path.display())]
    Access { path: PathBuf, source: std::io::Error },
    #[error("无法启动本地 IPC 服务")]
    Ipc { source: IpcError },
}

impl StartupError {
    pub fn runtime(source: std::io::Error) -> Self {
        Self::Runtime { source }
    }

    pub fn access(path: PathBuf, source: std::io::Error) -> Self {
        Self::Access { path, source }
    }

    pub fn ipc(source: IpcError) -> Self {
        Self::Ipc { source }
    }
}

pub fn run() -> Result<(), StartupError> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(StartupError::runtime)?;
    let context = runtime.block_on(GlobalContext::new());
    let directory = context.current_dir().to_path_buf();
    let cwd = directory.canonicalize().map_err(|source| StartupError::access(directory, source))?;
    let ipc = Ipc::start(runtime, &context).map_err(StartupError::ipc)?;
    let app = gpui_kit::application().with_assets(AllAssets);
    app.on_reopen(pet::open_workspace);
    app.run(move |cx| launch(cx, cwd, ipc));
    Ok(())
}

fn launch(cx: &mut App, cwd: PathBuf, ipc: Ipc) {
    gpui_kit::init(cx);
    ipc.init(cx);
    crate::keymap::init(cx);
    #[cfg(target_os = "macos")]
    install_menu(cx);
    let window = gpui_kit::open_window(window_options(cx), cx, |window, cx| {
        cx.new(|cx| WorkspaceView::new(cwd, Preferences::path(), aries_config(), window, cx))
    });
    let (handle, workspace) = match window {
        Ok(opened) => opened,
        Err(error) => {
            error::log("无法创建窗口", error.as_ref());
            cx.quit();
            return;
        },
    };
    install_quit(handle, &workspace, cx);
    configure_window(handle, &workspace, cx);
    cx.on_action(|_: &TogglePet, cx| pet::toggle(cx));
    cx.on_action(|_: &OpenWorkspace, cx| pet::open_workspace(cx));
    if let Err(error) = pet::init(&workspace, handle, cx)
        && let Err(window_error) = handle.update(cx, |_, window, cx| {
            error::report("无法显示桌面宠物", &error, window, cx);
        })
    {
        error::log("无法显示宠物错误", window_error.as_ref());
    }
    cx.activate(true);
}

#[cfg(target_os = "macos")]
fn install_menu(cx: &mut App) {
    use crate::{Hide, OpenSettings, ShowFilesSidebar, ShowGitSidebar, ShowSessionsSidebar};

    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.set_menus([Menu::new("Aries Desktop").items([
        MenuItem::action("打开 Aries…", OpenWorkspace::new()),
        MenuItem::action("显示 / 隐藏宠物", TogglePet::new()),
        MenuItem::action("设置…", OpenSettings),
        MenuItem::action("显示会话列表", ShowSessionsSidebar::new()),
        MenuItem::action("显示文件树", ShowFilesSidebar::new()),
        MenuItem::action("显示 Git 变更", ShowGitSidebar::new()),
        MenuItem::separator(),
        MenuItem::action("隐藏 Aries Desktop", Hide),
        MenuItem::separator(),
        MenuItem::action("退出 Aries Desktop", Quit),
    ])]);
}

fn window_options(cx: &mut App) -> WindowOptions {
    WindowOptions {
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
    }
}

fn install_quit(handle: AnyWindowHandle, workspace: &Entity<WorkspaceView>, cx: &mut App) {
    let workspace = workspace.downgrade();
    cx.on_action(move |_: &Quit, cx| {
        cx.stop_propagation();
        let workspace = workspace.clone();
        cx.defer(move |cx| {
            let Some(workspace) = workspace.upgrade() else {
                cx.quit();
                return;
            };
            if let Err(error) = handle.update(cx, |_, window, cx| {
                if let Err(error) = pet::show_main(window) {
                    error::report("无法恢复主窗口", &error, window, cx);
                    return;
                }
                workspace.update(cx, |workspace, cx| workspace.request_quit(window, cx));
            }) {
                error::log("无法请求退出", error.as_ref());
            }
        });
    });
}

fn configure_window(handle: AnyWindowHandle, workspace: &Entity<WorkspaceView>, cx: &mut App) {
    let workspace = workspace.downgrade();
    if let Err(error) = cx.update_window(handle, |_, window, cx| {
        window.on_window_should_close(cx, move |window, cx| {
            let workspace = workspace.clone();
            window.defer(cx, move |window, cx| {
                if pet::is_available(cx) {
                    if let Err(error) = pet::hide_main(window) {
                        error::report("无法隐藏主窗口", &error, window, cx);
                    }
                } else {
                    let _ =
                        workspace.update(cx, |workspace, cx| workspace.request_quit(window, cx));
                }
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
}
