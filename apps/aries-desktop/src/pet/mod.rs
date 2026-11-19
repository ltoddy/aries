mod appearance;
mod platform;

use std::time::{Duration, Instant};

use gpui_kit::base::Root;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::session::SessionFinished;
use crate::workspace::{SessionView, WorkspaceView};

const FRAME_INTERVAL: Duration = Duration::from_millis(125);
const COMPLETION_DURATION: Duration = Duration::from_secs(4);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PetStatus {
    Idle,
    Finished,
    Failed,
    Working,
    Permission,
}

impl PetStatus {
    fn idle() -> Self {
        Self::Idle
    }

    fn finished() -> Self {
        Self::Finished
    }

    fn from_session(view: &SessionView) -> Self {
        let execution = view.execution();
        if execution.permission().is_some() {
            Self::Permission
        } else if execution.conversation().is_busy() {
            Self::Working
        } else if execution.conversation().error().is_some() || execution.is_disconnected() {
            Self::Failed
        } else {
            Self::Idle
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Idle => "陪你写代码",
            Self::Finished => "任务完成",
            Self::Failed => "需要查看异常",
            Self::Working => "正在工作",
            Self::Permission => "需要确认授权",
        }
    }

    fn color(self, cx: &App) -> Hsla {
        match self {
            Self::Finished => cx.theme().success,
            Self::Failed => cx.theme().danger,
            Self::Permission => cx.theme().warning,
            Self::Working => cx.theme().primary,
            Self::Idle => cx.theme().foreground,
        }
    }
}

struct WatchedSession {
    view: WeakEntity<SessionView>,
    _changes: Subscription,
    _finished: Subscription,
}

impl WatchedSession {
    fn new(view: &Entity<SessionView>, cx: &mut Context<PetView>) -> Self {
        let changes = cx.observe(view, |this, _, cx| this.sync_status(cx));
        let finished = cx.subscribe(view, |this, view, _: &SessionFinished, cx| {
            this.completion = Some((view.entity_id(), Instant::now() + COMPLETION_DURATION));
            this.completion_timer = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(COMPLETION_DURATION).await;
                let _ = this.update(cx, |this, cx| {
                    this.completion = None;
                    this.sync_status(cx);
                });
            }));
            this.sync_status(cx);
        });
        Self { view: view.downgrade(), _changes: changes, _finished: finished }
    }
}

struct PetView {
    workspace: WeakEntity<WorkspaceView>,
    main_window: AnyWindowHandle,
    sessions: Vec<WatchedSession>,
    _workspace_changes: Subscription,
    _visibility_changes: Subscription,
    visible: bool,
    status: PetStatus,
    target: Option<EntityId>,
    completion: Option<(EntityId, Instant)>,
    started: Instant,
    animation: Option<Task<()>>,
    completion_timer: Option<Task<()>>,
    animation_enabled: bool,
}

impl PetView {
    fn new(
        workspace: Entity<WorkspaceView>,
        main_window: AnyWindowHandle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let changes = cx.observe(&workspace, |this, _, cx| this.sync_sessions(cx));
        let visibility = cx.observe_window_visibility(window, |this, visibility, _, cx| {
            this.visible = visibility.is_visible();
            if this.visible {
                this.start_animation(cx);
            } else {
                this.animation = None;
            }
        });
        let mut view = Self {
            workspace: workspace.downgrade(),
            main_window,
            sessions: Vec::new(),
            _workspace_changes: changes,
            _visibility_changes: visibility,
            visible: window.visibility().is_visible(),
            status: PetStatus::idle(),
            target: None,
            completion: None,
            started: Instant::now(),
            animation: None,
            completion_timer: None,
            animation_enabled: !platform::reduced_motion(),
        };
        view.sync_sessions(cx);
        view.start_animation(cx);
        view
    }

    fn sync_sessions(&mut self, cx: &mut Context<Self>) {
        let Some(workspace) = self.workspace.upgrade() else {
            return;
        };
        let views = workspace.read(cx).session_views();
        if !views.iter().map(Entity::entity_id).eq(self.sessions.iter().map(|s| s.view.entity_id()))
        {
            self.sessions = views.iter().map(|view| WatchedSession::new(view, cx)).collect();
        }
        self.sync_status(cx);
    }

    fn sync_status(&mut self, cx: &mut Context<Self>) {
        let mut status = PetStatus::idle();
        let mut target = None;
        let now = Instant::now();
        if self.completion.is_some_and(|(_, expires)| expires <= now) {
            self.completion = None;
        }
        for session in &self.sessions {
            let Some(view) = session.view.upgrade() else {
                continue;
            };
            let mut candidate = PetStatus::from_session(view.read(cx));
            if candidate == PetStatus::Idle
                && self.completion.is_some_and(|(id, _)| id == view.entity_id())
            {
                candidate = PetStatus::finished();
            }
            if candidate > status {
                status = candidate;
                target = Some(view.entity_id());
            }
        }
        if self.status != status || self.target != target {
            self.status = status;
            self.target = target;
            cx.notify();
        }
    }

    fn start_animation(&mut self, cx: &mut Context<Self>) {
        if !self.visible || !self.animation_enabled || self.animation.is_some() {
            return;
        }
        self.started = Instant::now();
        self.animation = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(FRAME_INTERVAL).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        }));
    }

    fn toggle_animation(&mut self, cx: &mut Context<Self>) {
        self.animation_enabled = !self.animation_enabled;
        if self.animation_enabled {
            self.start_animation(cx);
        } else {
            self.animation = None;
        }
        cx.notify();
    }

    fn open_workspace(&self, cx: &mut App) {
        let workspace = self.workspace.clone();
        let target = self.target;
        let main_window = self.main_window;
        cx.defer(move |cx| {
            cx.activate(true);
            if let Err(error) = main_window.update(cx, |_, window, cx| {
                platform::restore(window, cx);
                if let Some(workspace) = workspace.upgrade() {
                    workspace.update(cx, |workspace, cx| {
                        if let Some(target) = target {
                            workspace.focus_session_view(target, window, cx);
                        }
                    });
                }
                window.activate_window();
            }) {
                crate::error::log("无法打开桌宠对应会话", error.as_ref());
            }
        });
    }
}

impl Render for PetView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let phase = if self.animation_enabled { self.started.elapsed().as_secs_f32() } else { 0. };
        div()
            .id("desktop-pet")
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .child(
                div()
                    .id("pet-drag")
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .cursor(CursorStyle::OpenHand)
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                    .child(appearance::sheep(
                        phase,
                        self.status == PetStatus::Working,
                        self.status == PetStatus::Finished,
                    )),
            )
            .child(
                Button::new("pet-open-workspace")
                    .small()
                    .ghost()
                    .label(self.status.label())
                    .max_w_full()
                    .flex_shrink_0()
                    .text_color(self.status.color(cx))
                    .bg(cx.theme().background)
                    .tooltip(format!("{} · 打开对应会话；拖动小羊移动位置", self.status.label()))
                    .on_click(cx.listener(|this, _, _, cx| this.open_workspace(cx))),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, window, cx| {
                    let owner = cx.weak_entity();
                    let handle = window.window_handle();
                    platform::menu(
                        window,
                        !this.animation_enabled,
                        move |selected, cx| match selected {
                            Some(0) => {
                                let _ = owner.update(cx, |this, cx| this.open_workspace(cx));
                            },
                            Some(1) => {
                                let _ = owner.update(cx, |this, cx| this.toggle_animation(cx));
                            },
                            Some(2) => {
                                let _ = handle.update(cx, |_, window, _| window.remove_window());
                            },
                            _ => {},
                        },
                        cx,
                    );
                }),
            )
    }
}

struct PetWindow {
    handle: Option<AnyWindowHandle>,
}

impl PetWindow {
    fn new() -> Self {
        Self { handle: None }
    }
}

impl Global for PetWindow {}

pub fn init(main_window: AnyWindowHandle, workspace: &Entity<WorkspaceView>, cx: &mut App) {
    cx.set_global(PetWindow::new());
    let owner = workspace.downgrade();
    cx.on_action(move |_: &crate::ShowPet, cx| {
        if let Some(workspace) = owner.upgrade() {
            show(main_window, workspace, cx);
        }
    });
    show(main_window, workspace.clone(), cx);
}

fn show(main_window: AnyWindowHandle, workspace: Entity<WorkspaceView>, cx: &mut App) {
    if let Some(handle) = cx.global::<PetWindow>().handle
        && handle.update(cx, |_, window, cx| platform::show(window, cx)).is_ok()
    {
        return;
    }
    let Some(display) = cx.primary_display() else {
        return;
    };
    let visible = display.visible_bounds();
    let window_size = size(px(144.), px(136.));
    let origin = point(
        (visible.right() - window_size.width - px(24.)).max(visible.left()),
        (visible.bottom() - window_size.height - px(24.)).max(visible.top()),
    );
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, window_size))),
        titlebar: None,
        focus: false,
        show: !cfg!(target_os = "macos"),
        kind: WindowKind::PopUp,
        is_movable: true,
        app_owns_titlebar_drag: true,
        is_resizable: false,
        is_minimizable: false,
        display_id: Some(display.id()),
        window_background: WindowBackgroundAppearance::Transparent,
        ..WindowOptions::default()
    };
    match gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| PetView::new(workspace, main_window, window, cx))
    }) {
        Ok((handle, _)) => {
            cx.global_mut::<PetWindow>().handle = Some(handle);
            if let Err(error) = handle.update(cx, |_, window, cx| {
                Root::update(window, cx, |root, _, cx| {
                    root.style().background = Some(cx.theme().background.opacity(0.).into());
                    cx.notify();
                });
                platform::show(window, cx);
            }) {
                crate::error::log("无法显示桌宠窗口", error.as_ref());
            }
        },
        Err(error) => crate::error::log("无法创建桌宠窗口", error.as_ref()),
    }
}
