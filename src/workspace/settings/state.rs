use std::path::PathBuf;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::searchable_list::SearchableVec;
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{ActiveTheme, Disableable, Sizable, Theme, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::WorkspaceView;
use crate::workspace::settings::ui::open_shortcut_help;
use crate::workspace::settings::{Appearance, Preferences, SendKey, palette};

pub struct SettingsState {
    preferences_path: Option<PathBuf>,
    settings: Preferences,
    settings_error: Option<String>,
    settings_saving: bool,
    save_task: Option<Task<()>>,
    save_revision: usize,
    _subscriptions: Vec<Subscription>,
    font_select: Entity<SelectState<SearchableVec<String>>>,
    default_font_family: SharedString,
    mono_font_ratio: f32,
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
        let (settings, settings_error) = match preferences_path.as_deref() {
            Some(path) => Preferences::load_or_migrate(path, Preferences::legacy_path().as_deref()),
            None => (Preferences::default(), Some("未找到用户主目录，设置无法保存".into())),
        };
        let default_font_family = cx.theme().font_family.clone();
        let mono_font_ratio = cx.theme().mono_font_size / cx.theme().font_size;
        palette::apply(settings.appearance(), cx);
        Theme::update(cx, |theme| {
            theme.font_family = settings
                .font_family()
                .map(SharedString::from)
                .unwrap_or_else(|| default_font_family.clone());
            theme.font_size = px(settings.font_size());
            theme.mono_font_size = theme.font_size * mono_font_ratio;
        });
        let mut fonts = cx.text_system().all_font_names();
        if let Some(family) = settings.font_family() {
            fonts.push(family.to_owned());
        }
        fonts.sort_unstable();
        fonts.dedup();
        fonts.insert(0, "系统默认".into());
        let selected = settings.font_family().unwrap_or("系统默认").to_owned();
        let font_select = cx.new(|cx| {
            let mut state =
                SelectState::new(SearchableVec::new(fonts), None, window, cx).searchable(true);
            state.set_selected_value(&selected, window, cx);
            state
        });
        let font_subscription = cx.subscribe(&font_select, |this, _, event, cx| {
            if let SelectEvent::Confirm(Some(family)) = event {
                this.settings.set_font_family((family != "系统默认").then(|| family.clone()));
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
        Self {
            preferences_path,
            settings,
            settings_error,
            settings_saving: false,
            save_task: None,
            save_revision: 0,
            _subscriptions: vec![system_theme, quit, font_subscription],
            font_select,
            default_font_family,
            mono_font_ratio,
            host,
        }
    }
}

impl SettingsState {
    pub fn change_font_size(&mut self, step: i32, cx: &mut Context<Self>) {
        let size = if step == 0 { 16.0 } else { self.settings.font_size() + step as f32 };
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
            theme.mono_font_size = theme.font_size * self.mono_font_ratio;
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

    pub fn save_settings(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.preferences_path.clone() else {
            self.settings_error = Some("设置已生效，但未找到用户主目录，无法保存".into());
            let _ = self.host.update(cx, |_, cx| cx.notify());
            cx.notify();
            return;
        };
        let settings = self.settings.clone();
        self.save_revision += 1;
        let revision = self.save_revision;
        let previous = self.save_task.take();
        self.settings_error = None;
        self.settings_saving = true;
        self.save_task = Some(cx.spawn(async move |view, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            let result = cx.background_spawn(async move { settings.save(&path) }).await;
            let _ = view.update(cx, |this, cx| {
                if this.save_revision == revision {
                    this.settings_saving = false;
                    this.settings_error =
                        result.err().map(|error| format!("设置已生效，但未保存：{error}"));
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

impl SettingsState {
    pub fn open_settings(&self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let view = view.clone();
            dialog.title("设置").centered().width(cx.theme().font_size * 28.).content(
                move |content, _, cx| {
                    let Some(owner) = view.upgrade() else {
                        return content;
                    };
                    let this = owner.read(cx);
                    let appearance = this.settings.appearance();
                    let send_key = this.settings.send_key();
                    let send_keys = [SendKey::Enter, SendKey::ShiftEnter];
                    let send_key_view = view.clone();
                    let font_size = this.settings.font_size();
                    let font_select = this.font_select.clone();
                    let error = this.settings_error.clone();
                    let saving = this.settings_saving;
                    let path =
                        this.preferences_path.as_ref().map(|path| path.display().to_string());
                    let appearances = [Appearance::System, Appearance::Light, Appearance::Dark];
                    let theme_view = view.clone();
                    let retry = view.clone();
                    let font_smaller = view.clone();
                    let font_larger = view.clone();
                    let font_reset = view.clone();
                    content
                        .gap_4()
                        .child(div().font_weight(FontWeight::MEDIUM).child("外观"))
                        .child(
                            div().flex().flex_col().gap_2().child("主题").child(
                                RadioGroup::new("settings-theme")
                                    .selected_index(
                                        appearances.iter().position(|value| *value == appearance),
                                    )
                                    .children([
                                        Radio::new("theme-system").label("跟随系统"),
                                        Radio::new("theme-light").label("浅色"),
                                        Radio::new("theme-dark").label("深色"),
                                    ])
                                    .on_change(move |index, _, cx| {
                                        if let Some(appearance) = appearances.get(*index) {
                                            let _ = theme_view.update(cx, |this, cx| {
                                                this.set_appearance(*appearance, cx)
                                            });
                                        }
                                    }),
                            ),
                        )
                        .child(
                            div().flex().flex_col().gap_2().child("字体").child(
                                Select::new(&font_select)
                                    .id("settings-font-family")
                                    .accessibility_label("字体")
                                    .search_placeholder("搜索字体")
                                    .menu_max_h(rems(16.))
                                    .w_full(),
                            ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .child("字号")
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            Button::new("settings-font-smaller")
                                                .icon(IconName::Minus)
                                                .small()
                                                .disabled(font_size <= 12.0)
                                                .accessibility_label("减小字号")
                                                .on_click(move |_, _, cx| {
                                                    let _ = font_smaller.update(cx, |this, cx| {
                                                        this.change_font_size(-1, cx)
                                                    });
                                                }),
                                        )
                                        .child(
                                            Button::new("settings-font-reset")
                                                .label(format!("{font_size} px"))
                                                .small()
                                                .tooltip("恢复默认字号（16 px）")
                                                .accessibility_label("恢复默认字号")
                                                .on_click(move |_, _, cx| {
                                                    let _ = font_reset.update(cx, |this, cx| {
                                                        this.change_font_size(0, cx)
                                                    });
                                                }),
                                        )
                                        .child(
                                            Button::new("settings-font-larger")
                                                .icon(IconName::Plus)
                                                .small()
                                                .disabled(font_size >= 24.0)
                                                .accessibility_label("增大字号")
                                                .on_click(move |_, _, cx| {
                                                    let _ = font_larger.update(cx, |this, cx| {
                                                        this.change_font_size(1, cx)
                                                    });
                                                }),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("修改即时生效并自动保存。字号范围为 12–24 px。"),
                        )
                        .child(div().font_weight(FontWeight::MEDIUM).child("消息输入"))
                        .child(
                            div().flex().flex_col().gap_2().child("发送消息按键").child(
                                RadioGroup::new("settings-send-key")
                                    .selected_index(
                                        send_keys.iter().position(|key| *key == send_key),
                                    )
                                    .children([
                                        Radio::new("send-key-enter")
                                            .label("Enter 发送，Shift + Enter 换行"),
                                        Radio::new("send-key-shift-enter")
                                            .label("Shift + Enter 发送，Enter 换行"),
                                    ])
                                    .on_change(move |index, _, cx| {
                                        if let Some(send_key) = send_keys.get(*index) {
                                            let _ = send_key_view.update(cx, |this, cx| {
                                                this.set_send_key(*send_key, cx);
                                            });
                                        }
                                    }),
                            ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .child("快捷键")
                                .child(
                                    Button::new("shortcut-help")
                                        .label("快捷键…")
                                        .small()
                                        .accessibility_label("查看快捷键说明")
                                        .on_click(move |_, window, cx| {
                                            open_shortcut_help(send_key, window, cx)
                                        }),
                                ),
                        )
                        .when(saving, |content| {
                            content.child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("正在保存…"),
                            )
                        })
                        .when_some(error, |content, error| {
                            content.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div().text_sm().text_color(cx.theme().danger).child(error),
                                    )
                                    .child(
                                        Button::new("settings-retry")
                                            .label("重试保存")
                                            .small()
                                            .on_click(move |_, _, cx| {
                                                let _ = retry
                                                    .update(cx, |this, cx| this.save_settings(cx));
                                            }),
                                    ),
                            )
                        })
                        .when_some(path, |content, path| {
                            content.child(
                                div()
                                    .text_xs()
                                    .whitespace_normal()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("保存位置：{path}")),
                            )
                        })
                },
            )
        });
    }
}
