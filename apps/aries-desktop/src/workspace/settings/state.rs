use std::path::PathBuf;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::radio::{Radio, RadioGroup};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::searchable_list::{SearchableListItem, SearchableVec};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{ActiveTheme, Disableable, Selectable, Sizable, Theme, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::WorkspaceView;
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
    settings: Preferences,
    settings_error: Option<String>,
    settings_saving: bool,
    save_task: Option<Task<()>>,
    save_revision: usize,
    _subscriptions: Vec<Subscription>,
    font_select: Entity<SelectState<SearchableVec<FontOption>>>,
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
            Some(path) => Preferences::load_or_default(path),
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
        Self {
            page: SettingsPage::Appearance,
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
            let result = cx.background_spawn(async move { settings.save(path) }).await;
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
            dialog.title("设置").centered().width(cx.theme().font_size * 44.).content(
                move |content, _, cx| {
                    let Some(owner) = view.upgrade() else {
                        return content;
                    };
                    let this = owner.read(cx);
                    let page = this.page;
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
                    let navigation = div().flex().flex_col().gap_2().children(
                        [
                            SettingsPage::Appearance,
                            SettingsPage::MessageInput,
                            SettingsPage::Shortcuts,
                        ]
                        .into_iter()
                        .map(|target| {
                            let view = view.clone();
                            Button::new(target.id())
                                .ghost()
                                .label(target.label())
                                .w_full()
                                .justify_start()
                                .selected(page == target)
                                .on_click(move |_, _, cx| {
                                    let _ = view.update(cx, |this, cx| {
                                        this.page = target;
                                        cx.notify();
                                    });
                                    cx.refresh_windows();
                                })
                        }),
                    );
                    let page_content = div()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .p_4()
                        .child(div().font_weight(FontWeight::MEDIUM).child(page.label()))
                        .when(page == SettingsPage::Appearance, |content| {
                            content
                                .child(
                                    div().flex().flex_col().gap_2().child("主题").child(
                                        RadioGroup::new("settings-theme")
                                            .selected_index(
                                                appearances
                                                    .iter()
                                                    .position(|value| *value == appearance),
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
                                                            let _ = font_smaller.update(
                                                                cx,
                                                                |this, cx| {
                                                                    this.change_font_size(-1, cx)
                                                                },
                                                            );
                                                        }),
                                                )
                                                .child(
                                                    Button::new("settings-font-reset")
                                                        .label(format!("{font_size} px"))
                                                        .small()
                                                        .tooltip("恢复默认字号（16 px）")
                                                        .accessibility_label("恢复默认字号")
                                                        .on_click(move |_, _, cx| {
                                                            let _ = font_reset.update(
                                                                cx,
                                                                |this, cx| {
                                                                    this.change_font_size(0, cx)
                                                                },
                                                            );
                                                        }),
                                                )
                                                .child(
                                                    Button::new("settings-font-larger")
                                                        .icon(IconName::Plus)
                                                        .small()
                                                        .disabled(font_size >= 24.0)
                                                        .accessibility_label("增大字号")
                                                        .on_click(move |_, _, cx| {
                                                            let _ = font_larger.update(
                                                                cx,
                                                                |this, cx| {
                                                                    this.change_font_size(1, cx)
                                                                },
                                                            );
                                                        }),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child("字号范围为 12–24 px。"),
                                )
                        })
                        .when(page == SettingsPage::MessageInput, |content| {
                            content.child(
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
                        })
                        .when(page == SettingsPage::Shortcuts, |content| {
                            content.child(render_shortcuts(send_key, cx))
                        });
                    content
                        .gap_4()
                        .child(
                            div()
                                .flex()
                                .items_stretch()
                                .h(rems(25.))
                                .min_h_0()
                                .child(
                                    div()
                                        .w(rems(9.))
                                        .flex_shrink_0()
                                        .border_r_1()
                                        .border_color(cx.theme().border)
                                        .pr_3()
                                        .py_4()
                                        .child(navigation),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .min_h_0()
                                        .child(page_content)
                                        .overflow_y_scrollbar()
                                        .id(page.id()),
                                ),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("修改即时生效并自动保存。"),
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
