mod input;
mod paint;
mod session;

use std::path::PathBuf;

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::term::TermMode;
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, Bounds, Context, FocusHandle, Focusable, Keystroke, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Subscription, Task, Window, canvas, div, px, size,
};

gpui_kit::actions!(terminal, [CopyTerminal, PasteTerminal]);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        gpui_kit::KeyBinding::new("cmd-c", CopyTerminal, Some("Terminal")),
        gpui_kit::KeyBinding::new("cmd-v", PasteTerminal, Some("Terminal")),
    ]);
}

pub use self::session::Status;
use self::session::{Dimensions, Session, StartError, WriteError};
use crate::workspace::settings::typography;

pub struct TerminalView {
    directory: PathBuf,
    focus_handle: FocusHandle,
    session: Option<Session>,
    status: Status,
    notifications: async_channel::Sender<()>,
    _task: Task<()>,
    _quit_subscription: Subscription,
    _keystroke_subscription: Subscription,
    dimensions: Dimensions,
    cell_size: gpui_kit::Size<Pixels>,
    bounds: Bounds<Pixels>,
    cursor_bounds: Bounds<Pixels>,
    preedit: String,
    preedit_selection: std::ops::Range<usize>,
    input_error: Option<String>,
    scroll_remainder: f32,
    selecting: bool,
    visible: bool,
}

impl TerminalView {
    pub fn new(directory: impl Into<PathBuf>, cx: &mut Context<Self>) -> Self {
        let directory = directory.into();
        let (notifications, receiver) = async_channel::bounded(1);
        let task = cx.spawn(async move |this, cx| {
            while receiver.recv().await.is_ok() {
                if this
                    .update(cx, |this, cx| {
                        if let Some(session) = &this.session {
                            this.status = session.status();
                        }
                        if this.visible {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
                cx.background_executor().timer(std::time::Duration::from_millis(16)).await;
            }
        });
        let quit_subscription = cx.on_app_quit(|this, _| {
            let session = this.session.take();
            async move {
                if let Some(session) = session {
                    session.shutdown().await;
                }
            }
        });
        let entity = cx.weak_entity();
        let keystroke_subscription = cx.intercept_keystrokes(move |event, window, cx| {
            let Some(entity) = entity.upgrade() else {
                return;
            };
            if entity.read(cx).focus_handle.is_focused(window) {
                entity.update(cx, |this, cx| this.key_down(&event.keystroke, cx));
            }
        });
        let dimensions = Dimensions::new(80, 24);
        let mut view = Self {
            directory,
            focus_handle: cx.focus_handle().tab_stop(true),
            session: None,
            status: Status::starting(),
            notifications,
            _task: task,
            _quit_subscription: quit_subscription,
            _keystroke_subscription: keystroke_subscription,
            dimensions,
            cell_size: size(px(8.), px(18.)),
            bounds: Bounds::default(),
            cursor_bounds: Bounds::default(),
            preedit: String::new(),
            preedit_selection: 0..0,
            input_error: None,
            scroll_remainder: 0.,
            selecting: false,
            visible: true,
        };
        let _ = view.start();
        view
    }

    pub fn status(&self) -> Status {
        self.session.as_ref().map_or_else(|| self.status.clone(), Session::status)
    }

    pub fn set_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        if self.visible != visible {
            self.visible = visible;
            if !visible {
                self.selecting = false;
            }
            if visible {
                cx.notify();
            }
        }
    }

    pub fn directory(&self) -> &std::path::Path {
        &self.directory
    }

    pub fn restart(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.session.take();
        self.preedit.clear();
        self.preedit_selection = 0..0;
        self.input_error = None;
        self.scroll_remainder = 0.;
        self.selecting = false;
        if let Err(error) = self.start() {
            crate::error::report("重启 shell 失败", &error, window, cx);
        }
        window.focus(&self.focus_handle, cx);
        window.notify_text_input_changed(&self.focus_handle);
        cx.notify();
    }

    pub fn paste(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = self.paste_text(text, cx) {
            crate::error::report("粘贴到终端失败", &error, window, cx);
        }
    }

    fn paste_text(&mut self, text: &str, cx: &mut Context<Self>) -> Result<(), WriteError> {
        let session = self.session.as_ref().ok_or_else(WriteError::not_running)?;
        let bracketed = session.terminal()?.mode().contains(TermMode::BRACKETED_PASTE);
        let text = text.replace('\0', "");
        let bytes = if bracketed {
            format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', "")).into_bytes()
        } else {
            text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
        };
        self.send(bytes, cx)
    }

    fn start(&mut self) -> Result<(), StartError> {
        self.status = Status::starting();
        match Session::start(&self.directory, self.dimensions, self.notifications.clone()) {
            Ok(session) => {
                self.session = Some(session);
                Ok(())
            },
            Err(error) => {
                self.status = Status::failed(&error);
                Err(error)
            },
        }
    }

    fn send(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) -> Result<(), WriteError> {
        if bytes.is_empty() {
            return Ok(());
        }
        let result = (|| {
            let session = self.session.as_ref().ok_or_else(WriteError::not_running)?;
            session.write(bytes)?;
            let mut terminal = session.terminal()?;
            terminal.selection = None;
            terminal.scroll_display(Scroll::Bottom);
            self.selecting = false;
            Ok(())
        })();
        self.input_error = result.as_ref().err().map(ToString::to_string);
        cx.notify();
        result
    }

    fn key_down(&mut self, key: &Keystroke, cx: &mut Context<Self>) {
        if key.modifiers.platform || key.is_modifier_key() || !self.preedit.is_empty() {
            return;
        }
        let mode = self
            .session
            .as_ref()
            .and_then(|session| session.terminal().ok().map(|terminal| *terminal.mode()))
            .unwrap_or(TermMode::empty());
        if let Some(bytes) = self::input::key_bytes(key, mode) {
            let _ = self.send(bytes, cx);
            cx.stop_propagation();
        }
    }

    fn measure(&mut self, bounds: Bounds<Pixels>, window: &Window) {
        let font_size = typography::code_size_in_window(window);
        let cell_width = self::paint::cell_width(window, font_size);
        self.bounds = bounds;
        let cell_size = size(cell_width, font_size * 1.4);
        let metrics_changed = self.cell_size != cell_size;
        self.cell_size = cell_size;
        let dimensions = Dimensions::new(
            ((bounds.size.width / cell_width).floor() as usize).clamp(2, 1000),
            ((bounds.size.height / self.cell_size.height).floor() as usize).clamp(1, 1000),
        );
        if dimensions != self.dimensions || metrics_changed {
            if let Some(session) = &self.session {
                match session.resize(
                    dimensions,
                    f32::from(cell_width).ceil() as u16,
                    f32::from(self.cell_size.height).ceil() as u16,
                ) {
                    Ok(()) => self.dimensions = dimensions,
                    Err(error) => self.input_error = Some(error.to_string()),
                }
            } else {
                self.dimensions = dimensions;
            }
        }
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let paint_entity = entity.clone();
        let status = self.status();
        let message = match &status {
            Status::Starting => Some("正在启动 shell…".to_owned()),
            Status::Running => self.input_error.clone(),
            Status::Exited { code, signal } => Some(signal.as_ref().map_or_else(
                || format!("Shell 已退出（{code}）"),
                |signal| format!("Shell 已退出：{signal}"),
            )),
            Status::Failed(error) => Some(format!("终端启动或运行失败：{error}")),
        };
        div()
            .id("terminal-view")
            .key_context("Terminal")
            .track_focus(&self.focus_handle)
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|this, _: &CopyTerminal, window, cx| {
                this.copy_selection(window, cx);
            }))
            .on_action(cx.listener(|this, _: &PasteTerminal, window, cx| {
                if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                    this.paste(&text, window, cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus_handle, cx);
                    this.start_selection(event, cx);
                }),
            )
            .on_scroll(cx.listener(|this, event: &gpui_kit::ScrollEvent, _, cx| {
                if event.modifiers.platform {
                    return;
                }
                let delta =
                    event.delta.pixel_delta(this.cell_size.height).y / this.cell_size.height;
                this.scroll_remainder += delta;
                let lines = this.scroll_remainder.trunc() as i32;
                this.scroll_remainder -= lines as f32;
                if lines != 0 {
                    if let Some(session) = &this.session
                        && let Ok(mut terminal) = session.terminal()
                    {
                        terminal.scroll_display(Scroll::Delta(lines));
                    }
                    cx.notify();
                }
                cx.stop_propagation();
            }))
            .when_some(message, |view, message| {
                view.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .text_sm()
                        .child(div().flex_1().min_w_0().child(message))
                        .when(
                            matches!(status, Status::Exited { .. } | Status::Failed(_)),
                            |view| {
                                view.child(
                                    Button::new("restart-terminal")
                                        .ghost()
                                        .label("重启 shell")
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.restart(window, cx)
                                        })),
                                )
                            },
                        ),
                )
            })
            .child(
                canvas(
                    move |bounds, window, cx| {
                        entity.update(cx, |this, _| this.measure(bounds, window));
                    },
                    move |bounds, _, window, cx| {
                        paint_entity
                            .update(cx, |this, cx| self::paint::paint(this, bounds, window, cx));
                        let handle = paint_entity.read(cx).focus_handle.clone();
                        let mouse_entity = paint_entity.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                            if phase.bubble() {
                                mouse_entity.update(cx, |this, cx| {
                                    if this.selecting {
                                        if event.dragging() {
                                            this.update_selection(event.position, cx);
                                            cx.stop_propagation();
                                        } else {
                                            this.selecting = false;
                                        }
                                    }
                                });
                            }
                        });
                        let mouse_entity = paint_entity.clone();
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                            if phase.bubble() && event.button == MouseButton::Left {
                                mouse_entity.update(cx, |this, cx| {
                                    if this.selecting {
                                        this.update_selection(event.position, cx);
                                        this.selecting = false;
                                        cx.stop_propagation();
                                    }
                                });
                            }
                        });
                        window.handle_input(
                            &handle,
                            gpui_kit::ElementInputHandler::new(bounds, paint_entity),
                            cx,
                        );
                    },
                )
                .flex_1()
                .w_full()
                .min_h_0(),
            )
    }
}
