#![warn(missing_debug_implementations)]

mod acp;
mod chat;
mod preferences;

use gpui_kit::assets::AllAssets;
use gpui_kit::*;

use crate::acp::aries_config;
use crate::chat::view::TabsView;
use crate::preferences::Preferences;

gpui_kit::actions!(
    orbit,
    [
        Quit,
        NewSession,
        CloseTab,
        PreviousTab,
        NextTab,
        ZoomIn,
        ZoomOut,
        ResetZoom,
        PreviousUserMessage,
        NextUserMessage,
    ]
);

fn main() -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?.canonicalize()?;
    gpui_kit::application().with_assets(AllAssets).run(move |cx| {
        gpui_kit::init(cx);
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None), KeyBinding::new("ctrl-q", Quit, None)]);
        #[cfg(target_os = "macos")]
        cx.bind_keys([KeyBinding::new("cmd-n", NewSession, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        #[cfg(target_os = "macos")]
        cx.bind_keys([
            KeyBinding::new("cmd-w", CloseTab, None),
            KeyBinding::new("cmd-shift-[", PreviousTab, None),
            KeyBinding::new("cmd-shift-]", NextTab, None),
            KeyBinding::new("cmd-=", ZoomIn, None),
            KeyBinding::new("cmd-+", ZoomIn, None),
            KeyBinding::new("cmd--", ZoomOut, None),
            KeyBinding::new("cmd-0", ResetZoom, None),
            KeyBinding::new("cmd-alt-up", PreviousUserMessage, None),
            KeyBinding::new("cmd-alt-down", NextUserMessage, None),
        ]);
        #[cfg(not(target_os = "macos"))]
        cx.bind_keys([
            KeyBinding::new("ctrl-w", CloseTab, None),
            KeyBinding::new("ctrl-shift-[", PreviousTab, None),
            KeyBinding::new("ctrl-shift-]", NextTab, None),
            KeyBinding::new("ctrl-=", ZoomIn, None),
            KeyBinding::new("ctrl-+", ZoomIn, None),
            KeyBinding::new("ctrl--", ZoomOut, None),
            KeyBinding::new("ctrl-0", ResetZoom, None),
            KeyBinding::new("ctrl-alt-up", PreviousUserMessage, None),
            KeyBinding::new("ctrl-alt-down", NextUserMessage, None),
        ]);
        cx.bind_keys([
            KeyBinding::new("ctrl-tab", NextTab, None),
            KeyBinding::new("ctrl-shift-tab", PreviousTab, None),
        ]);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1200.), px(800.)),
                cx,
            ))),
            window_min_size: Some(size(px(760.), px(520.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Aries Orbit".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        if let Err(error) = gpui_kit::open_window(options, cx, |window, cx| {
            window.on_window_should_close(cx, |_, cx| {
                cx.quit();
                false
            });
            cx.new(|cx| TabsView::new(cwd, Preferences::path(), aries_config(), window, cx))
        }) {
            eprintln!("无法创建窗口：{error:#}");
            cx.quit();
        }
        cx.activate(true);
    });
    Ok(())
}
