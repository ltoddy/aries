use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::base::Root;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::ContextMenuExt;
use gpui_kit::component::{ActiveTheme, Sizable};
use gpui_kit::*;

use crate::pet::position::{Placement, Position};
use crate::pet::sprites::{CELL_HEIGHT, CELL_WIDTH, Clip, Sprites};
use crate::pet::{PetActivity, PetError, PetSession, native};
use crate::workspace::WorkspaceView;
use crate::{OpenWorkspace, Quit, TogglePet};

struct PetState {
    view: Entity<PetView>,
}

impl PetState {
    fn new(view: Entity<PetView>) -> Self {
        Self { view }
    }
}

impl Global for PetState {}

struct PetView {
    workspace: WeakEntity<WorkspaceView>,
    main_window: AnyWindowHandle,
    window: Option<AnyWindowHandle>,
    sprites: Option<Sprites>,
    sessions: Vec<PetSession>,
    completions: HashMap<EntityId, u64>,
    pending_completion: Option<PetSession>,
    target: Option<EntityId>,
    clip: Clip,
    frame: usize,
    transient: bool,
    position: Option<Position>,
    press: Option<Point<Pixels>>,
    waiting_focus: FocusHandle,
    dragged: bool,
    visible: bool,
    timer: Option<Task<()>>,
    save_task: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
    window_subscriptions: Vec<Subscription>,
}

impl std::fmt::Debug for PetView {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PetView")
            .field("window", &self.window)
            .field("target", &self.target)
            .field("clip", &self.clip)
            .field("frame", &self.frame)
            .finish_non_exhaustive()
    }
}

impl PetView {
    fn new(
        workspace: &Entity<WorkspaceView>,
        main_window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        let sessions = workspace.read(cx).pet_sessions(cx);
        let completions =
            sessions.iter().map(|session| (session.id(), session.completion())).collect();
        let position = match Position::load() {
            Ok(position) => position,
            Err(error) => {
                tracing::debug!(%error, "无法恢复宠物位置");
                None
            },
        };
        let observation = cx.observe(workspace, |this, workspace, cx| {
            this.sync(workspace.read(cx).pet_sessions(cx), cx);
        });
        let weak = cx.weak_entity();
        let displays = cx.observe_displays(move |_, cx| {
            let weak = weak.clone();
            cx.defer(move |cx| {
                if let Some(view) = weak.upgrade() {
                    restore_display(&view, cx);
                }
            });
        });
        let weak = cx.weak_entity();
        let quit = App::on_app_quit(cx, move |cx| {
            let position = weak.upgrade().and_then(|view| {
                view.update(cx, |this, _| {
                    this.timer = None;
                    this.save_task = None;
                    this.position.clone()
                })
            });
            cx.background_spawn(async move {
                if let Some(position) = position
                    && let Err(error) = position.save()
                {
                    tracing::debug!(%error, "无法保存宠物位置");
                }
            })
        });
        Self {
            workspace: workspace.downgrade(),
            main_window,
            window: None,
            sprites: None,
            sessions,
            completions,
            pending_completion: None,
            target: None,
            clip: Clip::idle(),
            frame: 0,
            transient: false,
            position,
            press: None,
            waiting_focus: cx.focus_handle().tab_stop(true),
            dragged: false,
            visible: false,
            timer: None,
            save_task: None,
            _subscriptions: vec![observation, displays, quit],
            window_subscriptions: Vec::new(),
        }
    }

    fn sync(&mut self, sessions: Vec<PetSession>, cx: &mut Context<Self>) {
        let completed = sessions
            .iter()
            .filter(|session| {
                self.completions
                    .get(&session.id())
                    .is_some_and(|completion| session.completion() > *completion)
            })
            .max_by_key(|session| session.activity() == PetActivity::Failed)
            .copied();
        self.completions =
            sessions.iter().map(|session| (session.id(), session.completion())).collect();
        self.sessions = sessions;
        if self.window.is_none() || !self.visible {
            self.pending_completion = None;
        } else {
            if self.pending_completion.is_some_and(|pending| {
                !self.sessions.iter().any(|session| session.id() == pending.id())
            }) {
                self.pending_completion = None;
            }
            if let Some(completed) = completed
                && self.pending_completion.is_none_or(|pending| {
                    pending.activity() != PetActivity::Failed
                        || completed.activity() == PetActivity::Failed
                })
            {
                self.pending_completion = Some(completed);
            }
        }
        if let Some(session) = self.busy_session() {
            if self.target != Some(session.id()) {
                self.target = Some(session.id());
                cx.notify();
            }
            let clip = match session.activity() {
                PetActivity::Waiting => Clip::waiting(),
                _ => Clip::running(),
            };
            self.set_clip(clip, false, cx);
            return;
        }
        if let Some(session) = self.pending_completion.take() {
            self.target = Some(session.id());
            let clip = if session.activity() == PetActivity::Failed {
                Clip::failed()
            } else {
                Clip::jumping()
            };
            self.set_clip(clip, true, cx);
            return;
        }
        if !self.transient {
            if !self.sessions.iter().any(|session| Some(session.id()) == self.target) {
                self.target = self.sessions.first().map(PetSession::id);
            }
            self.set_clip(Clip::idle(), false, cx);
        }
    }

    fn busy_session(&self) -> Option<PetSession> {
        self.sessions
            .iter()
            .filter(|session| {
                matches!(session.activity(), PetActivity::Waiting | PetActivity::Working)
            })
            .max_by_key(|session| {
                (session.activity() == PetActivity::Waiting, Some(session.id()) == self.target)
            })
            .copied()
    }

    fn set_clip(&mut self, clip: Clip, transient: bool, cx: &mut Context<Self>) {
        if self.clip == clip && self.transient == transient && !transient {
            return;
        }
        self.clip = clip;
        self.frame = 0;
        self.transient = transient;
        self.start_timer(cx);
        cx.notify();
    }

    fn delay(&self, cx: &App) -> Duration {
        if cx.reduce_motion() { self.clip.total_duration() } else { self.clip.duration(self.frame) }
    }

    fn start_timer(&mut self, cx: &mut Context<Self>) {
        self.timer = None;
        if self.window.is_none() || !self.visible {
            return;
        }
        let mut delay = self.delay(cx);
        self.timer = Some(cx.spawn(async move |weak, cx| {
            loop {
                cx.background_executor().timer(delay).await;
                let next = weak.update(cx, |this, cx| {
                    this.window?;
                    if cx.reduce_motion() {
                        if this.transient {
                            this.clip = Clip::idle();
                            this.frame = 0;
                            this.transient = false;
                            cx.notify();
                        }
                    } else {
                        this.frame += 1;
                        if this.frame == this.clip.frame_count() {
                            this.frame = 0;
                            if this.transient {
                                this.clip = Clip::idle();
                                this.transient = false;
                            }
                        }
                        cx.notify();
                    }
                    Some(this.delay(cx))
                });
                match next {
                    Ok(Some(next)) => delay = next,
                    _ => break,
                }
            }
        }));
    }

    fn hide(&mut self, cx: &mut Context<Self>) {
        self.timer = None;
        self.visible = false;
        self.press = None;
        self.transient = false;
        self.pending_completion = None;
        self.window_subscriptions.clear();
        let sprites = self.sprites.take();
        if let Some(handle) = self.window.take() {
            cx.defer(move |cx| {
                if let Err(error) = handle.update(cx, |_, window, _| {
                    if let Some(sprites) = sprites {
                        for image in sprites.frames() {
                            if let Err(error) = window.drop_image(image.clone()) {
                                tracing::debug!(%error, "无法释放宠物图片");
                            }
                        }
                    }
                    window.remove_window();
                }) {
                    PetView::report(PetError::window(error.to_string()));
                }
            });
        }
        cx.notify();
    }

    fn remember_position(&mut self, position: Position, cx: &mut Context<Self>) {
        if self.position.as_ref() == Some(&position) {
            return;
        }
        self.position = Some(position.clone());
        self.save_task = Some(cx.spawn(async move |_, cx| {
            cx.background_executor().timer(Duration::from_millis(300)).await;
            let result = cx.background_spawn(async move { position.save() }).await;
            if let Err(error) = result {
                tracing::debug!(%error, "无法保存宠物位置");
            }
        }));
    }

    fn report(error: PetError) {
        tracing::debug!(%error, "宠物不可用");
    }
}

impl Render for PetView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let frame = if cx.reduce_motion() { 0 } else { self.frame };
        let sprite = self.sprites.as_ref().map(|sprites| sprites.frame(self.clip, frame));
        div()
            .size_full()
            .relative()
            .flex()
            .items_end()
            .justify_end()
            .child(
                div()
                    .id("aries-pet")
                    .aria_label(match self.clip {
                        clip if clip == Clip::waiting() => "Aries pet: waiting for input",
                        clip if clip == Clip::running() => "Aries pet: working",
                        clip if clip == Clip::jumping() => "Aries pet: turn finished",
                        clip if clip == Clip::failed() => "Aries pet: turn failed",
                        _ => "Aries pet: open workspace or drag to move",
                    })
                    .w(px(CELL_WIDTH as f32))
                    .h(px(CELL_HEIGHT as f32))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            if window.default_prevented() {
                                return;
                            }
                            gpui_kit::base::GlobalState::suppress_text_selection(cx);
                            this.press = Some(event.position);
                            this.dragged = false;
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                        if event.dragging()
                            && !this.dragged
                            && this.press.is_some_and(|press| {
                                let delta = event.position - press;
                                f32::from(delta.x).abs() + f32::from(delta.y).abs() >= 4.
                            })
                        {
                            this.dragged = true;
                            gpui_kit::base::TextSelection::end(window, cx);
                            cx.stop_active_drag(window);
                            window.start_window_move();
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            let clicked = this.press.take().is_some() && !this.dragged;
                            match Position::from_window(window, cx) {
                                Ok(position) => this.remember_position(position, cx),
                                Err(error) => PetView::report(error),
                            }
                            if clicked {
                                if this.busy_session().is_none() {
                                    this.set_clip(Clip::waving(), true, cx);
                                }
                                cx.defer(open_workspace);
                            }
                        }),
                    )
                    .children(sprite.map(|sprite| img(sprite).size_full()))
                    .context_menu(|menu, _, _| {
                        menu.menu("隐藏宠物", Box::new(TogglePet::new()))
                            .menu("打开 Aries…", Box::new(OpenWorkspace::new()))
                            .separator()
                            .menu("退出 Aries", Box::new(Quit))
                    }),
            )
            .children(
                self.busy_session()
                    .filter(|session| session.activity() == PetActivity::Waiting)
                    .map(|session| {
                        let id = session.id();
                        div()
                            .absolute()
                            .top_2()
                            .right_2()
                            .p_1()
                            .rounded(cx.theme().radius)
                            .bg(cx.theme().popover)
                            .text_color(cx.theme().popover_foreground)
                            .border_1()
                            .border_color(cx.theme().border)
                            .child(
                                Button::new(("pet-waiting", id))
                                    .ghost()
                                    .small()
                                    .label("等待授权")
                                    .accessibility_label("等待授权，打开对应会话")
                                    .track_focus(&self.waiting_focus)
                                    .on_click(move |_, _, cx| open_session(Some(id), cx)),
                            )
                    }),
            )
    }
}

fn show(view: &Entity<PetView>, cx: &mut App) -> Result<(), PetError> {
    if view.read(cx).window.is_some() {
        return Ok(());
    }
    let sprites = Sprites::load()?;
    let placement = Position::bounds(view.read(cx).position.as_ref(), cx)?;
    view.update(cx, |this, cx| {
        this.sprites = Some(sprites);
        this.transient = false;
        this.pending_completion = None;
        this.clip = Clip::idle();
        this.frame = 0;
        cx.notify();
    });
    let options = window_options(&placement);
    let content = view.clone();
    let mut configuration = Ok(());
    let handle: AnyWindowHandle = cx
        .open_window(options, |window, cx| {
            configuration = native::configure(window);
            cx.new(|cx| {
                #[cfg(target_os = "macos")]
                window.set_native_titlebar_visibility(false);
                let mut root = Root::new(content, window, cx);
                root.style().background = Some(cx.theme().background.opacity(0.).into());
                root
            })
        })
        .map_err(|error| PetError::window(error.to_string()))?
        .into();
    view.update(cx, |this, _| {
        this.window = Some(handle);
        this.visible = false;
        this.position = Some(placement.position().clone());
    });
    configuration?;
    let close_weak = view.downgrade();
    let visibility_weak = view.downgrade();
    let bounds_weak = view.downgrade();
    let display_weak = view.downgrade();
    let (subscriptions, initial_visibility) = handle
        .update(cx, |_, window, cx| {
            window.on_window_should_close(cx, move |_, cx| {
                let weak = close_weak.clone();
                cx.defer(move |cx| {
                    let _ = weak.update(cx, |this, cx| {
                        if this.window == Some(handle) {
                            this.hide(cx);
                        }
                    });
                });
                false
            });
            let visibility = window.observe_window_visibility(move |visibility, _, cx| {
                let weak = visibility_weak.clone();
                cx.defer(move |cx| {
                    let _ = weak.update(cx, |this, cx| {
                        if this.window == Some(handle) {
                            this.visible = visibility.is_visible();
                            if !this.visible {
                                this.pending_completion = None;
                            }
                            this.start_timer(cx);
                        }
                    });
                });
            });
            let bounds = Root::update(window, cx, |root, window, cx| {
                root.style().background = Some(cx.theme().background.opacity(0.).into());
                cx.notify();
                cx.observe_window_bounds(window, move |_, window, cx| {
                    let position = Position::from_window(window, cx);
                    let weak = bounds_weak.clone();
                    cx.defer(move |cx| {
                        let _ = weak.update(cx, |this, cx| {
                            if this.window == Some(handle) {
                                match position {
                                    Ok(position) => this.remember_position(position, cx),
                                    Err(error) => PetView::report(error),
                                }
                            }
                        });
                    });
                })
            });
            let display = window.observe_window_display(move |window, cx| {
                let position = Position::from_window(window, cx);
                let weak = display_weak.clone();
                cx.defer(move |cx| {
                    let _ = weak.update(cx, |this, cx| {
                        if this.window == Some(handle) {
                            match position {
                                Ok(position) => this.remember_position(position, cx),
                                Err(error) => PetView::report(error),
                            }
                        }
                    });
                });
            });
            (vec![visibility, bounds, display], window.visibility())
        })
        .map_err(|error| PetError::window(error.to_string()))?;
    view.update(cx, |this, cx| {
        this.window_subscriptions = subscriptions;
        this.visible = initial_visibility.is_visible();
        if let Some(workspace) = this.workspace.upgrade() {
            let sessions = workspace.read(cx).pet_sessions(cx);
            this.completions =
                sessions.iter().map(|session| (session.id(), session.completion())).collect();
            this.sync(sessions, cx);
        }
        this.start_timer(cx);
        cx.notify();
    });
    Ok(())
}

fn window_options(placement: &Placement) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(placement.bounds())),
        display_id: Some(placement.display_id()),
        kind: WindowKind::PopUp,
        titlebar: None,
        focus: false,
        is_resizable: false,
        is_minimizable: false,
        app_owns_titlebar_drag: true,
        window_background: WindowBackgroundAppearance::Transparent,
        ..WindowOptions::default()
    }
}

fn restore_display(view: &Entity<PetView>, cx: &mut App) {
    if view.read(cx).window.is_none() {
        return;
    }
    match Position::bounds(view.read(cx).position.as_ref(), cx) {
        Ok(placement) if Some(placement.position()) != view.read(cx).position.as_ref() => {
            view.update(cx, |this, cx| {
                this.hide(cx);
                this.position = Some(placement.position().clone());
            });
            if let Err(error) = show(view, cx) {
                view.update(cx, |this, cx| this.hide(cx));
                PetView::report(error);
            }
        },
        Err(error) => {
            view.update(cx, |this, cx| this.hide(cx));
            PetView::report(error);
        },
        _ => {},
    }
}

pub fn init(workspace: &Entity<WorkspaceView>, main_window: AnyWindowHandle, cx: &mut App) {
    if cx.has_global::<PetState>() {
        return;
    }
    let view = cx.new(|cx| PetView::new(workspace, main_window, cx));
    cx.set_global(PetState::new(view.clone()));
    if let Err(error) = show(&view, cx) {
        view.update(cx, |this, cx| this.hide(cx));
        PetView::report(error);
    }
}

pub fn is_available(cx: &App) -> bool {
    cx.try_global::<PetState>().is_some_and(|state| state.view.read(cx).window.is_some())
}

pub fn toggle(cx: &mut App) {
    cx.defer(toggle_view);
}

fn toggle_view(cx: &mut App) {
    let Some(view) = cx.try_global::<PetState>().map(|state| state.view.clone()) else {
        return;
    };
    if view.read(cx).window.is_some() {
        let main_window = view.read(cx).main_window;
        let restored = main_window
            .update(cx, |_, window, _| native::show_main(window))
            .map_err(|error| PetError::window(error.to_string()))
            .and_then(|result| result);
        if let Err(error) = restored {
            PetView::report(error);
            return;
        }
        cx.activate(true);
        view.update(cx, |this, cx| this.hide(cx));
    } else if let Err(error) = show(&view, cx) {
        view.update(cx, |this, cx| this.hide(cx));
        PetView::report(error);
    }
}

pub fn open_workspace(cx: &mut App) {
    let target = cx.try_global::<PetState>().and_then(|state| state.view.read(cx).target);
    open_session(target, cx);
}

fn open_session(target: Option<EntityId>, cx: &mut App) {
    let Some((workspace, window)) = cx.try_global::<PetState>().map(|state| {
        let pet = state.view.read(cx);
        (pet.workspace.clone(), pet.main_window)
    }) else {
        return;
    };
    cx.defer(move |cx| {
        cx.activate(true);
        let result = window
            .update(cx, |_, window, cx| {
                native::show_main(window)?;
                if let Some(id) = target {
                    workspace
                        .update(cx, |workspace, cx| workspace.focus_pet_session(id, window, cx))
                        .map_err(|error| PetError::window(error.to_string()))?;
                }
                Ok(())
            })
            .map_err(|error| PetError::window(error.to_string()))
            .and_then(|result| result);
        if let Err(error) = result {
            PetView::report(error);
        }
    });
}
