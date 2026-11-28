use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::ContextMenuExt;
use gpui_kit::component::{ActiveTheme, Sizable, Theme, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

use crate::ipc::Ipc;
#[cfg(target_os = "linux")]
use crate::pet::Mover;
use crate::pet::position::{Desktop, Placement, Position};
use crate::pet::sprites::{Clip, Sprites};
use crate::pet::{PetActivity, PetError, PetSession, native};
use crate::workspace::{WorkspaceView, number};
use crate::{OpenWorkspace, Quit};

const USAGE_DEBOUNCE: Duration = Duration::from_millis(100);
const SLEEP_AFTER: Duration = Duration::from_secs(60);

#[cfg(target_os = "linux")]
#[derive(Clone)]
struct X11Drag {
    origin: Point<Pixels>,
    press: Point<Pixels>,
    position: Position,
}

#[cfg(target_os = "linux")]
impl X11Drag {
    fn new(origin: Point<Pixels>, press: Point<Pixels>, position: Position) -> Self {
        Self { origin, press, position }
    }

    fn origin(&self) -> Point<Pixels> {
        self.origin
    }

    fn press(&self) -> Point<Pixels> {
        self.press
    }

    fn position(&self) -> &Position {
        &self.position
    }
}

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
    usages: HashMap<EntityId, u64>,
    today_usage: Option<u64>,
    usage_task: Option<Task<()>>,
    pending_completion: Option<PetSession>,
    target: Option<EntityId>,
    clip: Clip,
    frame: usize,
    scale: f32,
    font_size: Pixels,
    transient: bool,
    position: Option<Position>,
    press: Option<Point<Pixels>>,
    waiting_focus: FocusHandle,
    dragged: bool,
    visible: bool,
    anchored: bool,
    display_uuid: Option<String>,
    input_regions: Vec<Bounds<Pixels>>,
    #[cfg(target_os = "linux")]
    mover: Option<Mover>,
    #[cfg(target_os = "linux")]
    x11_drag: Option<X11Drag>,
    timer: Option<Task<()>>,
    sleep_task: Option<Task<()>>,
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
        let usages = sessions.iter().map(|session| (session.id(), session.usage())).collect();
        let position = match Position::load() {
            Ok(position) => position,
            Err(error) => {
                tracing::debug!(%error, "无法恢复宠物位置");
                None
            },
        };
        let scale = workspace.read(cx).preferences(cx).pet_scale();
        let font_size = cx.theme().font_size;
        let theme = cx.observe_global::<Theme>(|this, cx| this.apply_layout(this.scale, cx));
        let observation = cx.observe(workspace, |this, workspace, cx| {
            let scale = workspace.read(cx).preferences(cx).pet_scale();
            this.apply_layout(scale, cx);
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
                    this.sleep_task = None;
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
            usages,
            today_usage: None,
            usage_task: None,
            pending_completion: None,
            target: None,
            clip: Clip::idle(),
            frame: 0,
            scale,
            font_size,
            transient: false,
            position,
            press: None,
            waiting_focus: cx.focus_handle().tab_stop(true),
            dragged: false,
            visible: false,
            anchored: false,
            display_uuid: None,
            input_regions: Vec::new(),
            #[cfg(target_os = "linux")]
            mover: None,
            #[cfg(target_os = "linux")]
            x11_drag: None,
            timer: None,
            sleep_task: None,
            save_task: None,
            _subscriptions: vec![observation, theme, displays, quit],
            window_subscriptions: Vec::new(),
        }
    }

    fn sync(&mut self, sessions: Vec<PetSession>, cx: &mut Context<Self>) {
        let usage_changed = sessions.iter().any(|session| {
            self.usages.get(&session.id()).is_some_and(|usage| session.usage() > *usage)
        });
        self.usages = sessions.iter().map(|session| (session.id(), session.usage())).collect();
        if usage_changed && self.window.is_some() {
            self.refresh_usage(USAGE_DEBOUNCE, cx);
        }
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
        if !self.transient && !self.dragging() && self.clip != Clip::sleep() {
            if !self.sessions.iter().any(|session| Some(session.id()) == self.target) {
                self.target = self.sessions.first().map(PetSession::id);
            }
            self.set_clip(Clip::idle(), false, cx);
        }
    }

    fn refresh_usage(&mut self, delay: Duration, cx: &mut Context<Self>) {
        let client = cx.global::<Ipc>().client();
        self.usage_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            let result = cx.background_spawn(async move { client.get_today_token_usage() }).await;
            let usage = match result {
                Ok(response) => response.token_usage.unwrap_or_default(),
                Err(error) => {
                    tracing::debug!(%error, "无法读取当日用量");
                    let _ = this.update(cx, |this, cx| {
                        this.today_usage = None;
                        cx.notify();
                    });
                    return;
                },
            };
            let total = if usage.total_tokens > 0 {
                usage.total_tokens
            } else {
                usage.input_tokens.saturating_add(usage.output_tokens)
            };
            let _ = this.update(cx, |this, cx| {
                this.today_usage = Some(total);
                cx.notify();
            });
        }));
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
        if clip == Clip::idle() {
            self.schedule_sleep(cx);
        } else {
            self.sleep_task = None;
        }
        self.start_timer(cx);
        cx.notify();
    }

    fn settled_clip(&self) -> Clip {
        match self.busy_session() {
            Some(session) if session.activity() == PetActivity::Waiting => Clip::waiting(),
            Some(_) => Clip::running(),
            None => Clip::idle(),
        }
    }

    fn settle(&mut self, cx: &mut Context<Self>) {
        let clip = self.settled_clip();
        self.set_clip(clip, false, cx);
    }

    fn end_transient(&mut self, cx: &mut Context<Self>) {
        self.transient = false;
        self.frame = 0;
        self.clip = self.settled_clip();
        if self.clip == Clip::idle() {
            self.schedule_sleep(cx);
        } else {
            self.sleep_task = None;
        }
    }

    fn schedule_sleep(&mut self, cx: &mut Context<Self>) {
        self.sleep_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SLEEP_AFTER).await;
            let _ = this.update(cx, |this, cx| {
                if this.clip == Clip::idle()
                    && !this.transient
                    && !this.dragging()
                    && this.window.is_some()
                    && this.visible
                {
                    this.set_clip(Clip::sleep(), false, cx);
                }
            });
        }));
    }

    fn dragging(&self) -> bool {
        self.press.is_some() && self.dragged
    }

    fn run(&mut self, delta: Pixels, cx: &mut Context<Self>) {
        if delta > px(0.) {
            self.set_clip(Clip::run_right(), false, cx);
        } else if delta < px(0.) {
            self.set_clip(Clip::run_left(), false, cx);
        }
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
                            this.end_transient(cx);
                            cx.notify();
                        }
                    } else {
                        this.frame += 1;
                        if this.frame == this.clip.frame_count() {
                            this.frame = 0;
                            if this.transient {
                                this.end_transient(cx);
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

    fn apply_layout(&mut self, scale: f32, cx: &mut Context<Self>) {
        let font_size = cx.theme().font_size;
        if self.scale == scale && self.font_size == font_size {
            return;
        }
        self.scale = scale;
        self.font_size = font_size;
        if self.window.is_none() {
            cx.notify();
            return;
        }
        let view = cx.weak_entity();
        cx.defer(move |cx| {
            let Some(view) = view.upgrade() else {
                return;
            };
            view.update(cx, |this, cx| this.hide(cx));
            if let Err(error) = show(&view, cx) {
                view.update(cx, |this, cx| this.hide(cx));
                PetView::report(error);
            }
        });
    }

    fn hide(&mut self, cx: &mut Context<Self>) {
        self.timer = None;
        self.sleep_task = None;
        self.usage_task = None;
        self.visible = false;
        self.press = None;
        self.transient = false;
        self.pending_completion = None;
        self.window_subscriptions.clear();
        #[cfg(target_os = "linux")]
        {
            self.mover = None;
            self.x11_drag = None;
        }
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
        self.position = Some(position);
        self.persist_position(cx);
    }

    fn persist_position(&mut self, cx: &mut Context<Self>) {
        let Some(position) = self.position.clone() else {
            return;
        };
        self.save_task = Some(cx.spawn(async move |_, cx| {
            cx.background_executor().timer(Duration::from_millis(300)).await;
            let result = cx.background_spawn(async move { position.save() }).await;
            if let Err(error) = result {
                tracing::debug!(%error, "无法保存宠物位置");
            }
        }));
    }

    fn origin(&mut self, surface: Size<Pixels>, cell: Size<Pixels>) -> Point<Pixels> {
        let max = point(
            (surface.width - cell.width).max(px(0.)),
            (surface.height - cell.height).max(px(0.)),
        );
        if !self.anchored {
            return point(max.x / 2., max.y);
        }
        let origin = self
            .position
            .as_ref()
            .map_or(max, |position| point(px(position.x()), px(position.y())));
        let origin = point(origin.x.clamp(px(0.), max.x), origin.y.clamp(px(0.), max.y));
        let uuid = self
            .position
            .as_ref()
            .map(|position| position.display_uuid().to_string())
            .or_else(|| self.display_uuid.clone());
        if let Some(uuid) = uuid {
            self.position = Some(Position::new(origin, uuid));
        }
        origin
    }

    fn press(&mut self, event: &MouseDownEvent, window: &Window, cx: &mut Context<Self>) {
        if window.default_prevented() {
            return;
        }
        gpui_kit::base::GlobalState::suppress_text_selection(cx);
        self.press = Some(event.position);
        self.dragged = false;
        #[cfg(target_os = "linux")]
        if let (Some(_), Some(position)) = (&self.mover, self.position.clone()) {
            let origin = window.window_bounds().get_bounds().origin;
            self.x11_drag = Some(X11Drag::new(origin, event.position, position));
        }
    }

    fn drag(&mut self, event: &MouseMoveEvent, window: &Window, cx: &mut Context<Self>) {
        if !event.dragging() {
            return;
        }
        #[cfg(target_os = "linux")]
        if let Some(drag) = self.x11_drag.clone() {
            let delta = event.position - drag.press();
            if delta.x != px(0.) || delta.y != px(0.) {
                self.drag_x11(drag, delta, window, cx);
            }
            return;
        }
        let Some(last) = self.press else {
            return;
        };
        let delta = event.position - last;
        if delta.x == px(0.) && delta.y == px(0.) {
            return;
        }
        self.press = Some(event.position);
        let Some(position) = self.position.clone() else {
            return;
        };
        let cell = Position::cell(self.scale);
        let surface = window.bounds().size;
        let max = point(
            (surface.width - cell.width).max(px(0.)),
            (surface.height - cell.height).max(px(0.)),
        );
        let origin = point(
            px(position.x() + f32::from(delta.x)).clamp(px(0.), max.x),
            px(position.y() + f32::from(delta.y)).clamp(px(0.), max.y),
        );
        self.position = Some(Position::new(origin, position.display_uuid().to_string()));
        self.dragged = true;
        self.run(delta.x, cx);
        cx.notify();
    }

    #[cfg(target_os = "linux")]
    fn drag_x11(
        &mut self,
        drag: X11Drag,
        delta: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some(mover) = &self.mover else {
            return;
        };
        let origin = drag.origin() + delta;
        if let Err(error) = mover.move_to(origin, window.scale_factor()) {
            PetView::report(error);
            return;
        }
        self.position = Some(drag.position().moved(delta));
        self.dragged = true;
        self.run(delta.x, cx);
    }

    fn window_moved(&self) -> bool {
        #[cfg(target_os = "linux")]
        {
            self.mover.is_some()
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    fn render_status(
        &self,
        region: Bounds<Pixels>,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<Anchored> {
        let waiting =
            self.busy_session().filter(|session| session.activity() == PetActivity::Waiting);
        if self.today_usage.is_none() && waiting.is_none() {
            return None;
        }
        let gap = rems(0.25).to_pixels(window.rem_size());
        let entity = cx.entity();
        Some(
            anchored()
                .anchor(Anchor::BottomCenter)
                .attach(region, Anchor::TopCenter)
                .offset(point(px(0.), -gap))
                .child(
                    v_flex()
                        .relative()
                        .flex_shrink_0()
                        .max_w(window.bounds().size.width - gap * 2.)
                        .rounded(cx.theme().radius_tokens().md)
                        .bg(cx.theme().popover)
                        .text_color(cx.theme().popover_foreground)
                        .border_1()
                        .border_color(cx.theme().border)
                        .children(self.today_usage.map(|today| {
                            h_flex()
                                .id("pet-usage")
                                .aria_label(format!("今日用量：{today} tokens"))
                                .justify_center()
                                .gap_2()
                                .px_3()
                                .py_1()
                                .text_xs()
                                .whitespace_nowrap()
                                .child(div().text_color(cx.theme().muted_foreground).child("今日"))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::MEDIUM)
                                        .font_family(cx.theme().mono_font_family.clone())
                                        .child(number::compact(today)),
                                )
                                .child(
                                    div().text_color(cx.theme().muted_foreground).child("tokens"),
                                )
                        }))
                        .children(waiting.map(|session| {
                            let id = session.id();
                            div()
                                .p_1()
                                .when(self.today_usage.is_some(), |this| {
                                    this.border_t_1().border_color(cx.theme().border)
                                })
                                .child(
                                    Button::new(("pet-waiting", id))
                                        .ghost()
                                        .small()
                                        .w_full()
                                        .label("等待授权")
                                        .accessibility_label("等待授权，打开对应会话")
                                        .track_focus(&self.waiting_focus)
                                        .on_click(move |_, _, cx| open_session(Some(id), cx)),
                                )
                        }))
                        .when(self.anchored, |this| {
                            this.child(
                                canvas(
                                    move |bounds, window, cx| {
                                        entity.update(cx, |this, _| {
                                            let regions = [region, bounds];
                                            if this.input_regions.as_slice() != regions {
                                                window.set_input_region(Some(&regions));
                                                this.input_regions = regions.to_vec();
                                            }
                                        });
                                    },
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .size_full(),
                            )
                        }),
                ),
        )
    }

    fn report(error: PetError) {
        tracing::debug!(%error, "宠物不可用");
    }
}

impl Render for PetView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let frame = if cx.reduce_motion() { 0 } else { self.frame };
        let sprite = self.sprites.as_ref().map(|sprites| sprites.frame(self.clip, frame));
        let cell = Position::cell(self.scale);
        window.set_rem_size(cx.theme().font_size);
        let origin = self.origin(window.bounds().size, cell);
        let region = Bounds::new(origin, cell);
        let status = self.render_status(region, window, cx);
        if self.anchored && status.is_none() && self.input_regions.as_slice() != [region] {
            window.set_input_region(Some(&[region]));
            self.input_regions = vec![region];
        }
        div()
            .size_full()
            .relative()
            .font_family(cx.theme().font_family.clone())
            .text_color(cx.theme().foreground)
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if this.press.is_none() {
                    return;
                }
                if this.anchored {
                    this.drag(event, window, cx);
                    return;
                }
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
                    if std::mem::take(&mut this.dragged) {
                        this.settle(cx);
                    }
                    #[cfg(target_os = "linux")]
                    {
                        this.x11_drag = None;
                    }
                    if this.anchored || this.window_moved() {
                        this.persist_position(cx);
                    } else {
                        match Position::from_window(window, cx) {
                            Ok(position) => this.remember_position(position, cx),
                            Err(error) => PetView::report(error),
                        }
                    }
                    if clicked {
                        if this.busy_session().is_none() {
                            this.set_clip(Clip::waving(), true, cx);
                        }
                        cx.defer(open_workspace);
                    }
                }),
            )
            .child(
                div()
                    .id("aries-pet")
                    .absolute()
                    .left(origin.x)
                    .top(origin.y)
                    .w(cell.width)
                    .h(cell.height)
                    .aria_label(match self.clip {
                        clip if clip == Clip::waiting() => "Aries pet: waiting for input",
                        clip if clip == Clip::running() => "Aries pet: working",
                        clip if clip == Clip::jumping() => "Aries pet: turn finished",
                        clip if clip == Clip::failed() => "Aries pet: turn failed",
                        clip if clip == Clip::run_right() => "Aries pet: running right",
                        clip if clip == Clip::run_left() => "Aries pet: running left",
                        clip if clip == Clip::sleep() => "Aries pet: sleeping",
                        _ => "Aries pet: open workspace or drag to move",
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.press(event, window, cx)
                        }),
                    )
                    .children(sprite.map(|sprite| img(sprite).size_full()))
                    .context_menu(|menu, _, _| {
                        menu.menu("打开 Aries…", Box::new(OpenWorkspace::new()))
                            .separator()
                            .menu("退出 Aries", Box::new(Quit))
                    }),
            )
            .children(status)
    }
}

fn show(view: &Entity<PetView>, cx: &mut App) -> Result<(), PetError> {
    if view.read(cx).window.is_some() {
        return Ok(());
    }
    let sprites = Sprites::load()?;
    let saved = view.read(cx).position.clone();
    view.update(cx, |this, cx| {
        this.sprites = Some(sprites);
        this.transient = false;
        this.pending_completion = None;
        this.clip = Clip::idle();
        this.frame = 0;
        this.schedule_sleep(cx);
        cx.notify();
    });
    let content = view.clone();
    let mut configuration = Ok(());
    let opened = open_pet_window(saved.as_ref(), &content, &mut configuration, cx);
    let (handle, desktop, placement) = opened?;
    let anchored = desktop.is_some();
    #[cfg(target_os = "linux")]
    let mover = if anchored {
        None
    } else {
        handle.update(cx, |_, window, _| Mover::new(window)).ok().flatten()
    };
    let display_uuid = desktop.as_ref().map(|desktop| desktop.uuid().to_string());
    let position = match (&desktop, &placement) {
        (Some(_), _) => saved,
        (None, Some(placement)) => Some(placement.position().clone()),
        (None, None) => return Err(PetError::window("无法确定宠物窗口位置".into())),
    };
    view.update(cx, |this, _| {
        this.window = Some(handle);
        this.anchored = anchored;
        this.display_uuid = display_uuid;
        this.visible = false;
        this.position = position;
        this.input_regions.clear();
        #[cfg(target_os = "linux")]
        {
            this.mover = mover;
            this.x11_drag = None;
        }
    });
    configuration?;
    let close_weak = view.downgrade();
    let visibility_weak = view.downgrade();
    let display_weak = view.downgrade();
    let entity = view.clone();
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
            let bounds = entity.update(cx, |_, cx| {
                cx.observe_window_bounds(window, move |this, window, cx| {
                    if this.window == Some(handle) && !this.anchored {
                        match Position::from_window(window, cx) {
                            Ok(position) => this.remember_position(position, cx),
                            Err(error) => PetView::report(error),
                        }
                    }
                })
            });
            let display = window.observe_window_display(move |window, cx| {
                let position = Position::from_window(window, cx);
                let weak = display_weak.clone();
                cx.defer(move |cx| {
                    let _ = weak.update(cx, |this, cx| {
                        if this.window == Some(handle) && !this.anchored {
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
            this.usages = sessions.iter().map(|session| (session.id(), session.usage())).collect();
            this.sync(sessions, cx);
        }
        this.refresh_usage(Duration::ZERO, cx);
        this.start_timer(cx);
        cx.notify();
    });
    Ok(())
}

#[cfg(target_os = "linux")]
fn open_pet_window(
    saved: Option<&Position>,
    content: &Entity<PetView>,
    configuration: &mut Result<(), PetError>,
    cx: &mut App,
) -> Result<(AnyWindowHandle, Option<Desktop>, Option<Placement>), PetError> {
    if wayland_session() {
        let desktop = Position::desktop(saved, cx)?;
        let options = layer_shell_options(&desktop);
        if let Ok(handle) =
            cx.open_window(options, build_pet_window(content.clone(), configuration))
        {
            return Ok((AnyWindowHandle::from(handle), Some(desktop), None));
        }
    }
    let (handle, placement) = open_popup(saved, content, configuration, cx)?;
    Ok((handle, None, Some(placement)))
}

#[cfg(not(target_os = "linux"))]
fn open_pet_window(
    saved: Option<&Position>,
    content: &Entity<PetView>,
    configuration: &mut Result<(), PetError>,
    cx: &mut App,
) -> Result<(AnyWindowHandle, Option<Desktop>, Option<Placement>), PetError> {
    let (handle, placement) = open_popup(saved, content, configuration, cx)?;
    Ok((handle, None, Some(placement)))
}

fn open_popup(
    saved: Option<&Position>,
    content: &Entity<PetView>,
    configuration: &mut Result<(), PetError>,
    cx: &mut App,
) -> Result<(AnyWindowHandle, Placement), PetError> {
    let scale = content.read(cx).scale;
    let placement = Position::bounds(saved, scale, cx)?;
    let handle = cx
        .open_window(
            window_options(&placement, scale),
            build_pet_window(content.clone(), configuration),
        )
        .map(AnyWindowHandle::from)
        .map_err(|error| PetError::window(error.to_string()))?;
    Ok((handle, placement))
}

fn build_pet_window(
    content: Entity<PetView>,
    configuration: &mut Result<(), PetError>,
) -> impl FnOnce(&mut Window, &mut App) -> Entity<PetView> + '_ {
    move |window, _| {
        *configuration = native::configure(window);
        #[cfg(target_os = "macos")]
        window.set_native_titlebar_visibility(false);
        content
    }
}

#[cfg(target_os = "linux")]
fn wayland_session() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("WAYLAND_SOCKET").is_some()
}

#[cfg(target_os = "linux")]
fn layer_shell_options(desktop: &Desktop) -> WindowOptions {
    use gpui_kit::layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions};

    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(desktop.bounds())),
        display_id: Some(desktop.display_id()),
        kind: WindowKind::LayerShell(LayerShellOptions {
            namespace: PET_APP_ID.into(),
            layer: Layer::Top,
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            exclusive_zone: Some(px(0.)),
            exclusive_edge: None,
            margin: None,
            keyboard_interactivity: KeyboardInteractivity::None,
        }),
        ..base_window_options()
    }
}

fn base_window_options() -> WindowOptions {
    WindowOptions {
        titlebar: None,
        focus: false,
        is_resizable: false,
        is_minimizable: false,
        app_owns_titlebar_drag: true,
        app_id: Some(PET_APP_ID.into()),
        window_background: WindowBackgroundAppearance::Transparent,
        ..WindowOptions::default()
    }
}

fn window_options(placement: &Placement, scale: f32) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(placement.bounds())),
        display_id: Some(placement.display_id()),
        kind: WindowKind::PopUp,
        window_min_size: Some(Position::cell(scale)),
        ..base_window_options()
    }
}

const PET_APP_ID: &str = "aries-pet";

fn restore_display(view: &Entity<PetView>, cx: &mut App) {
    if view.read(cx).window.is_none() || view.read(cx).anchored {
        return;
    }
    match Position::bounds(view.read(cx).position.as_ref(), view.read(cx).scale, cx) {
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
    let visible = workspace.read(cx).preferences(cx).pet_visible();
    let view = cx.new(|cx| PetView::new(workspace, main_window, cx));
    cx.set_global(PetState::new(view.clone()));
    if !visible {
        return;
    }
    if let Err(error) = show(&view, cx) {
        view.update(cx, |this, cx| this.hide(cx));
        PetView::report(error);
    }
}

#[cfg(target_os = "macos")]
pub fn is_available(cx: &App) -> bool {
    cx.try_global::<PetState>().is_some_and(|state| state.view.read(cx).window.is_some())
}

pub fn set_visible(visible: bool, cx: &mut App) {
    cx.defer(move |cx| apply_visibility(visible, cx));
}

fn apply_visibility(visible: bool, cx: &mut App) {
    let Some(view) = cx.try_global::<PetState>().map(|state| state.view.clone()) else {
        return;
    };
    if view.read(cx).window.is_some() == visible {
        return;
    }
    if visible {
        if let Err(error) = show(&view, cx) {
            view.update(cx, |this, cx| this.hide(cx));
            PetView::report(error);
        }
        return;
    }
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
