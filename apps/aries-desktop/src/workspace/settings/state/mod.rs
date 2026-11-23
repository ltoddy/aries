use std::path::PathBuf;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::searchable_list::{SearchableListItem, SearchableVec};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{ActiveTheme, Disableable, Selectable, Sizable, Theme};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::error::{details, log, report};
use crate::workspace::WorkspaceView;
use crate::workspace::settings::preferences::SavePreferencesError;
use crate::workspace::settings::typography::{BASE_FONT_SIZE, CODE_FONT_SIZE};
use crate::workspace::settings::ui::render_shortcuts;
use crate::workspace::settings::{Appearance, Preferences, SendKey, palette};

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsPage {
    Appearance,
    MessageInput,
    Shortcuts,
}

impl SettingsPage {
    fn id(self) -> &'static str {
        match self {
            Self::Appearance => "settings-page-appearance",
            Self::MessageInput => "settings-page-message-input",
            Self::Shortcuts => "settings-page-shortcuts",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Appearance => "外观",
            Self::MessageInput => "消息输入",
            Self::Shortcuts => "快捷键",
        }
    }
}

#[derive(Clone)]
struct FontOption {
    label: SharedString,
    value: Option<String>,
}

impl FontOption {
    fn new(value: Option<String>) -> Self {
        let label = value.as_deref().unwrap_or("系统默认").to_owned().into();
        Self { label, value }
    }
}

impl SearchableListItem for FontOption {
    type Value = Option<String>;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

pub struct SettingsState {
    page: SettingsPage,
    preferences_path: Option<PathBuf>,
    window: AnyWindowHandle,
    settings: Preferences,
    settings_error: Option<String>,
    settings_saving: bool,
    save_task: Option<Task<()>>,
    save_revision: usize,
    _subscriptions: Vec<Subscription>,
    font_select: Entity<SelectState<SearchableVec<FontOption>>>,
    default_font_family: SharedString,
    host: WeakEntity<WorkspaceView>,
}

impl SettingsState {
    pub fn preferences(&self) -> &Preferences {
        &self.settings
    }

    pub fn error(&self) -> Option<&str> {
        self.settings_error.as_deref()
    }

    pub fn new(
        preferences_path: Option<PathBuf>,
        host: WeakEntity<WorkspaceView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (settings, load_error) = match preferences_path.as_deref() {
            Some(path) => Preferences::load_or_default(path),
            None => (Preferences::default(), None),
        };
        let default_font_family = cx.theme().font_family.clone();
        palette::apply(settings.appearance(), cx);
        Theme::update(cx, |theme| {
            theme.font_family = settings
                .font_family()
                .map(SharedString::from)
                .unwrap_or_else(|| default_font_family.clone());
            theme.font_size = px(settings.font_size());
            theme.mono_font_size = px(CODE_FONT_SIZE);
        });
        let mut fonts = cx.text_system().all_font_names();
        if let Some(family) = settings.font_family() {
            fonts.push(family.to_owned());
        }
        fonts.sort_unstable();
        fonts.dedup();
        let mut fonts: Vec<_> =
            fonts.into_iter().map(|family| FontOption::new(Some(family))).collect();
        fonts.insert(0, FontOption::new(None));
        let selected = settings.font_family().map(str::to_owned);
        let font_select = cx.new(|cx| {
            let mut state =
                SelectState::new(SearchableVec::new(fonts), None, window, cx).searchable(true);
            state.set_selected_value(&selected, window, cx);
            state
        });
        let font_subscription = cx.subscribe(&font_select, |this, _, event, cx| {
            if let SelectEvent::Confirm(Some(family)) = event {
                this.settings.set_font_family(family.clone());
                this.apply_typography(cx);
            }
        });
        let system_theme = cx.observe_window_appearance(window, |this, window, cx| {
            if this.settings.appearance() == Appearance::System {
                palette::sync(window.appearance().into(), cx);
                let _ = this.host.update(cx, |_, cx| cx.notify());
                cx.notify();
            }
        });
        let quit = cx.on_app_quit(|this, _| {
            let task = this.save_task.take();
            async move {
                if let Some(task) = task {
                    task.await;
                }
            }
        });
        let mut state = Self {
            page: SettingsPage::Appearance,
            preferences_path,
            window: window.window_handle(),
            settings,
            settings_error: None,
            settings_saving: false,
            save_task: None,
            save_revision: 0,
            _subscriptions: vec![system_theme, quit, font_subscription],
            font_select,
            default_font_family,
            host,
        };
        if let Some(error) = load_error {
            state.report_error("无法读取设置，已使用默认值；原文件未修改", error, cx);
        } else if state.preferences_path.is_none() {
            state.report_error("设置无法保存", SavePreferencesError::missing_home(), cx);
        }
        state
    }
}

impl SettingsState {
    pub fn change_font_size(&mut self, step: i32, cx: &mut Context<Self>) {
        let size = if step == 0 { BASE_FONT_SIZE } else { self.settings.font_size() + step as f32 };
        self.settings.set_font_size(size);
        self.apply_typography(cx);
    }

    pub fn apply_typography(&mut self, cx: &mut Context<Self>) {
        Theme::update(cx, |theme| {
            theme.font_family = self
                .settings
                .font_family()
                .map(SharedString::from)
                .unwrap_or_else(|| self.default_font_family.clone());
            theme.font_size = px(self.settings.font_size());
            theme.mono_font_size = px(CODE_FONT_SIZE);
        });
        self.save_settings(cx);
        let _ = self.host.update(cx, |host, cx| host.remeasure_timelines(cx));
        cx.notify();
    }

    pub fn set_send_key(&mut self, send_key: SendKey, cx: &mut Context<Self>) {
        self.settings.set_send_key(send_key);
        let _ = self.host.update(cx, |host, cx| host.notify_settings_tabs(cx));
        self.save_settings(cx);
    }

    pub fn set_appearance(&mut self, appearance: Appearance, cx: &mut Context<Self>) {
        self.settings.set_appearance(appearance);
        palette::apply(appearance, cx);
        self.save_settings(cx);
    }

    fn report_error(
        &mut self,
        title: &'static str,
        error: impl std::error::Error + 'static,
        cx: &mut Context<Self>,
    ) {
        self.settings_error = Some(format!("{title}：{}", details(&error)));
        let window = self.window;
        cx.defer(move |cx| {
            if cx.update_window(window, |_, window, cx| report(title, &error, window, cx)).is_err()
            {
                log(title, &error);
            }
        });
    }

    pub fn save_settings(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.preferences_path.clone() else {
            self.report_error("设置已生效，但未保存", SavePreferencesError::missing_home(), cx);
            let _ = self.host.update(cx, |host, cx| host.notify_settings_sidebar(cx));
            cx.refresh_windows();
            let _ = self.host.update(cx, |_, cx| cx.notify());
            cx.notify();
            return;
        };
        let settings = self.settings.clone();
        self.save_revision += 1;
        let revision = self.save_revision;
        let previous = self.save_task.take();
        self.settings_saving = true;
        self.save_task = Some(cx.spawn(async move |view, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            let result = cx.background_spawn(async move { settings.save(path) }).await;
            let _ = view.update(cx, |this, cx| {
                if this.save_revision == revision {
                    this.settings_saving = false;
                    match result {
                        Ok(()) => this.settings_error = None,
                        Err(error) => this.report_error("设置已生效，但未保存", error, cx),
                    }
                    let _ = this.host.update(cx, |host, cx| host.notify_settings_sidebar(cx));
                    cx.refresh_windows();
                    let _ = this.host.update(cx, |_, cx| cx.notify());
                    cx.notify();
                }
            });
        }));
        let _ = self.host.update(cx, |host, cx| host.notify_settings_sidebar(cx));
        cx.refresh_windows();
        let _ = self.host.update(cx, |_, cx| cx.notify());
        cx.notify();
    }
}

mod page;
