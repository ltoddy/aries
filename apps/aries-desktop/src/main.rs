mod acp;
mod terminal;
mod workspace;

use anyhow::Context as _;
use gpui_kit::assets::AllAssets;
use gpui_kit::*;

use crate::acp::aries_config;
use crate::workspace::WorkspaceView;
use crate::workspace::settings::Preferences;

gpui_kit::actions!(
    orbit,
    [
        Quit,
        NewSession,
        CloseTab,
        PreviousTab,
        NextTab,
        PreviousUserMessage,
        NextUserMessage,
        ToggleTerminal,
    ]
);

fn main() -> anyhow::Result<()> {
    let directory = std::env::current_dir().context("无法读取启动目录")?;
    let cwd = directory
        .canonicalize()
        .with_context(|| format!("无法访问启动目录 {}", directory.display()))?;
    gpui_kit::application().with_assets(AllAssets).run(move |cx| {
        gpui_kit::init(cx);
        crate::terminal::init(cx);
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None), KeyBinding::new("ctrl-q", Quit, None)]);
        #[cfg(target_os = "macos")]
        cx.bind_keys([
            KeyBinding::new("cmd-n", NewSession, None),
            KeyBinding::new("cmd-f", ToggleTerminal, None),
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
        cx.set_menus(
            [Menu::new("Aries Desktop").items([MenuItem::action("退出 Aries Desktop", Quit)])],
        );
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1200.), px(800.)),
                cx,
            ))),
            window_min_size: Some(size(px(760.), px(520.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Aries Desktop".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        if let Err(error) = gpui_kit::open_window(options, cx, |window, cx| {
            window.on_window_should_close(cx, |_, cx| {
                cx.quit();
                false
            });
            cx.new(|cx| WorkspaceView::new(cwd, Preferences::path(), aries_config(), window, cx))
        }) {
            eprintln!("无法创建窗口：{error:#}");
            cx.quit();
        }
        cx.activate(true);
    });
    Ok(())
}
