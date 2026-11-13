use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use aries_persistence::{TokenUsage, TokenUsageRepository};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::searchable_list::SearchableListItem;
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, IndexPath, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp, Zoned};
use thiserror::Error;

use crate::workspace::number;

pub struct ReturnToSessions;

impl ReturnToSessions {
    fn new() -> Self {
        Self
    }
}

#[derive(Clone, Copy, Default)]
struct DayUsage {
    input: u64,
    output: u64,
    total: u64,
    cache: u64,
    reasoning: u64,
    records: u64,
}

impl DayUsage {
    fn add(&mut self, record: &TokenUsage) {
        self.input = self.input.saturating_add(record.input_tokens);
        self.output = self.output.saturating_add(record.output_tokens);
        self.total = self.total.saturating_add(if record.total_tokens == 0 {
            record.input_tokens.saturating_add(record.output_tokens)
        } else {
            record.total_tokens
        });
        self.cache = self.cache.saturating_add(record.cached_input_tokens);
        self.reasoning = self.reasoning.saturating_add(record.reasoning_tokens);
        self.records += 1;
    }
}

struct UsageData {
    projects: BTreeMap<String, BTreeMap<Date, DayUsage>>,
    all: BTreeMap<Date, DayUsage>,
    today_projects: BTreeMap<String, DayUsage>,
    today_all: Option<DayUsage>,
}

impl UsageData {
    fn from_records(
        records: impl IntoIterator<Item = TokenUsage>,
        today_records: impl IntoIterator<Item = TokenUsage>,
        timezone: TimeZone,
    ) -> Self {
        let mut projects: BTreeMap<String, BTreeMap<Date, DayUsage>> = BTreeMap::new();
        let mut all: BTreeMap<Date, DayUsage> = BTreeMap::new();
        for record in records {
            let date = record.created_at.to_zoned(timezone.clone()).date();
            projects.entry(record.cwd.clone()).or_default().entry(date).or_default().add(&record);
            all.entry(date).or_default().add(&record);
        }
        let mut today_projects: BTreeMap<String, DayUsage> = BTreeMap::new();
        let mut today_all: Option<DayUsage> = None;
        for record in today_records {
            today_projects.entry(record.cwd.clone()).or_default().add(&record);
            today_all.get_or_insert_default().add(&record);
        }
        Self { projects, all, today_projects, today_all }
    }
}

#[derive(Clone)]
struct ProjectOption {
    label: SharedString,
    value: Option<String>,
}

impl ProjectOption {
    fn new(value: Option<String>) -> Self {
        let label = value.as_deref().unwrap_or("全部项目").to_owned().into();
        Self { label, value }
    }
}

impl SearchableListItem for ProjectOption {
    type Value = Option<String>;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.value
    }
}

enum LoadState {
    Idle,
    Loading,
    Ready,
    Failed(String),
}

pub struct UsageView {
    data: Option<UsageData>,
    state: LoadState,
    year: i16,
    project: Option<String>,
    selected: Date,
    project_select: Entity<SelectState<Vec<ProjectOption>>>,
    focus: FocusHandle,
    scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
    _task: Option<Task<()>>,
}

impl EventEmitter<ReturnToSessions> for UsageView {}

impl UsageView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let today = Zoned::now().date();
        let project_select = cx.new(|cx| {
            SelectState::new(vec![ProjectOption::new(None)], Some(IndexPath::new(0)), window, cx)
                .searchable(true)
        });
        let subscriptions = vec![cx.subscribe(
            &project_select,
            |this, _, event: &SelectEvent<Vec<ProjectOption>>, cx| {
                let SelectEvent::Confirm(value) = event;
                this.project = value.clone().flatten();
                cx.notify();
            },
        )];
        Self {
            data: None,
            state: LoadState::Idle,
            year: today.year(),
            project: None,
            selected: today,
            project_select,
            focus: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            _subscriptions: subscriptions,
            _task: None,
        }
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        self.scroll_to_selected();
        if matches!(self.state, LoadState::Idle) {
            self.refresh(window, cx);
        }
    }

    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self._task = None;
        let today = Zoned::now().date();
        if self.year != today.year() {
            self.year = today.year();
            self.selected = today;
            self.scroll_to_selected();
        }
        self.data = None;
        self.state = LoadState::Loading;
        let year = self.year;
        let background = cx.background_executor().spawn(async move { load_usage(year) });
        self._task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = background.await;
            let _ = this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(data) => {
                        let mut projects = vec![ProjectOption::new(None)];
                        projects.extend(
                            data.projects
                                .keys()
                                .cloned()
                                .map(|project| ProjectOption::new(Some(project))),
                        );
                        if let Some(project) = &this.project
                            && !data.projects.contains_key(project)
                        {
                            projects.push(ProjectOption::new(Some(project.clone())));
                        }
                        this.project_select.update(cx, |select, cx| {
                            select.set_items(projects, window, cx);
                            select.set_selected_value(&this.project, window, cx);
                        });
                        this.data = Some(data);
                        this.state = LoadState::Ready;
                    },
                    Err(error) => {
                        crate::error::log("无法读取用量", &error);
                        this.state = LoadState::Failed(crate::error::details(&error));
                    },
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn days(&self) -> Option<&BTreeMap<Date, DayUsage>> {
        let data = self.data.as_ref()?;
        match &self.project {
            Some(project) => data.projects.get(project),
            None => Some(&data.all),
        }
    }

    fn first_day(&self) -> Date {
        self.selected.saturating_sub(Span::new().days(self.selected.day_of_year() - 1))
    }

    fn last_day(&self) -> Date {
        let first = self.first_day();
        first
            .saturating_add(Span::new().days(first.days_in_year() - 1))
            .min(Zoned::now().date())
            .max(self.first_day())
    }

    fn navigate(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if key.modifiers.platform
            || key.modifiers.control
            || key.modifiers.alt
            || key.modifiers.shift
        {
            return;
        }
        let offset = match key.key.as_str() {
            "left" => -7,
            "right" => 7,
            "up" => -1,
            "down" => 1,
            "home" => {
                self.selected = self.first_day();
                0
            },
            "end" => {
                self.selected = self.last_day();
                0
            },
            _ => return,
        };
        if let Ok(date) = self.selected.checked_add(Span::new().days(offset)) {
            self.selected = date.clamp(self.first_day(), self.last_day());
        }
        self.scroll_to_selected();
        cx.stop_propagation();
        cx.notify();
    }

    fn scroll_to_selected(&self) {
        let first = self.first_day();
        let week = (i32::from(self.selected.day_of_year()) - 1
            + i32::from(first.weekday().to_sunday_zero_offset()))
            / 7;
        self.scroll.scroll_to_item(week as usize);
    }

    fn render_heatmap(&self, cx: &mut Context<Self>) -> AnyElement {
        let first = self.first_day();
        let today = Zoned::now().date();
        let offset = i32::from(first.weekday().to_sunday_zero_offset());
        let count = i32::from(first.days_in_year());
        let weeks = (offset + count + 6) / 7;
        let days = self.days();
        let maximum = days
            .into_iter()
            .flat_map(|days| days.iter())
            .filter(|(date, _)| date.year() == self.year && **date <= today)
            .map(|(_, usage)| usage.total)
            .max()
            .unwrap_or(0);
        let primary = cx.theme().primary;
        let border = cx.theme().border;
        let muted = cx.theme().muted;
        div()
            .flex()
            .items_start()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .pt_7()
                    .border_t_1()
                    .border_color(cx.theme().background)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .children(
                        ["日", "一", "二", "三", "四", "五", "六"]
                            .into_iter()
                            .map(|label| div().h_4().child(label)),
                    ),
            )
            .child(
                div()
                    .id("usage-heatmap")
                    .role(Role::Grid)
                    .aria_label(format!("{}年用量热力图", self.year))
                    .aria_description("左右移动七天，上下移动一天，Home 和 End 跳转首尾日期。")
                    .flex_1()
                    .min_w_0()
                    .overflow_x_scroll()
                    .track_focus(&self.focus)
                    .track_scroll(&self.scroll)
                    .border_1()
                    .border_color(border)
                    .rounded(cx.theme().radius)
                    .p_2()
                    .focus_visible(move |style| style.border_color(primary))
                    .on_key_down(cx.listener(|this, event, _, cx| this.navigate(event, cx)))
                    .flex()
                    .items_start()
                    .gap_1()
                    .children((0..weeks).map(|week| {
                        let start = week * 7 - offset;
                        let month = (0..7)
                            .find_map(|row| {
                                let ordinal = start + row;
                                if !(0..count).contains(&ordinal) {
                                    return None;
                                }
                                let date = first.checked_add(Span::new().days(ordinal)).ok()?;
                                (date.day() == 1).then(|| format!("{}月", date.month()))
                            })
                            .unwrap_or_default();
                        div()
                            .id(SharedString::from(format!("usage-week-{}-{week}", self.year)))
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div().relative().h_4().w_4().child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .left_0()
                                        .w_12()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(month),
                                ),
                            )
                            .children((0..7).map(|row| {
                                let ordinal = start + row;
                                let date = first.checked_add(Span::new().days(ordinal)).ok();
                                let date = date
                                    .filter(|date| (0..count).contains(&ordinal) && *date <= today);
                                let Some(date) = date else {
                                    return div().size_4().into_any_element();
                                };
                                let usage = days.and_then(|days| days.get(&date));
                                let color = usage
                                    .filter(|usage| usage.total > 0)
                                    .map(|usage| {
                                        let level = if maximum == 0 {
                                            1.
                                        } else {
                                            usage.total as f32 / maximum as f32
                                        };
                                        primary.opacity(if level <= 0.25 {
                                            0.25
                                        } else if level <= 0.5 {
                                            0.45
                                        } else if level <= 0.75 {
                                            0.7
                                        } else {
                                            1.
                                        })
                                    })
                                    .unwrap_or(muted);
                                let label = usage
                                    .map(|usage| {
                                        format!("{date}：{} tokens", number::compact(usage.total))
                                    })
                                    .unwrap_or_else(|| format!("{date}：无记录"));
                                div()
                                    .id(SharedString::from(format!("usage-day-{date}")))
                                    .role(Role::GridCell)
                                    .aria_label(label.clone())
                                    .when(self.selected == date, |cell| {
                                        cell.aria_active_descendant()
                                    })
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(label.clone()).build(window, cx)
                                    })
                                    .size_4()
                                    .rounded_sm()
                                    .border_1()
                                    .p(rems(0.0625))
                                    .border_color(if self.selected == date {
                                        primary
                                    } else {
                                        border
                                    })
                                    .hover(move |style| {
                                        style.border_color(primary).bg(primary.opacity(0.2))
                                    })
                                    .child(div().size_full().rounded_sm().bg(color))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.selected = date;
                                        this.focus.focus(window, cx);
                                        cx.notify();
                                    }))
                                    .into_any_element()
                            }))
                    })),
            )
            .into_any_element()
    }

    fn render_summary(&self, cx: &App) -> AnyElement {
        let today = Zoned::now().date();
        let days = self.days();
        let range: Vec<_> = days
            .into_iter()
            .flat_map(|days| days.iter())
            .filter(|(date, _)| date.year() == self.year && **date <= today)
            .collect();
        let total = range.iter().fold(0u64, |total, (_, day)| total.saturating_add(day.total));
        let active = range.iter().filter(|(_, day)| day.total > 0).count();
        let today_usage = self.data.as_ref().and_then(|data| match &self.project {
            Some(project) => data.today_projects.get(project),
            None => data.today_all.as_ref(),
        });
        let loaded = self.data.is_some();
        let values = [
            (
                "所选范围累计",
                if loaded { format!("{} tokens", number::compact(total)) } else { "—".into() },
                if loaded {
                    format!("{} tokens", number::compact(total))
                } else {
                    "尚未读取".into()
                },
            ),
            (
                "今日 · 当前项目筛选",
                today_usage
                    .map(|usage| format!("{} tokens", number::compact(usage.total)))
                    .unwrap_or_else(|| if loaded { "无记录".into() } else { "—".into() }),
                today_usage
                    .map(|usage| format!("{} tokens", number::compact(usage.total)))
                    .unwrap_or_else(|| {
                        if loaded { "无记录".into() } else { "尚未读取".into() }
                    }),
            ),
            (
                "活跃天数 · 已记录用量 > 0",
                if loaded { number::compact(active as u64) } else { "—".into() },
                if loaded { active.to_string() } else { "尚未读取".into() },
            ),
        ];
        div()
            .flex()
            .flex_wrap()
            .gap_6()
            .children(values.into_iter().map(|(label, value, exact)| {
                div()
                    .flex_1()
                    .min_w(rems(10.))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_left()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(label),
                    )
                    .child(
                        div()
                            .id(SharedString::from(format!("usage-summary-{label}")))
                            .aria_label(format!("{label}：{exact}"))
                            .tooltip(move |window, cx| {
                                Tooltip::new(exact.clone()).build(window, cx)
                            })
                            .text_left()
                            .text_lg()
                            .font_weight(FontWeight::MEDIUM)
                            .child(value),
                    )
            }))
            .into_any_element()
    }

    fn render_detail(&self, cx: &App) -> AnyElement {
        let usage = self.days().and_then(|days| days.get(&self.selected));
        div()
            .flex()
            .flex_col()
            .gap_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .pt_4()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .child(format!("{} · 日明细", self.selected)),
            )
            .child(match usage {
                Some(usage) => div()
                    .flex()
                    .flex_wrap()
                    .gap_4()
                    .children(
                        [
                            ("总量", usage.total),
                            ("输入", usage.input),
                            ("输出", usage.output),
                            ("缓存输入", usage.cache),
                            ("推理", usage.reasoning),
                            ("记录数", usage.records),
                        ]
                        .into_iter()
                        .map(|(label, value)| {
                            let exact = format!("{label}：{}", number::compact(value));
                            div()
                                .w_32()
                                .flex_shrink_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .text_left()
                                        .child(label),
                                )
                                .child(
                                    div()
                                        .id(SharedString::from(format!("usage-detail-{label}")))
                                        .aria_label(exact.clone())
                                        .tooltip(move |window, cx| {
                                            Tooltip::new(exact.clone()).build(window, cx)
                                        })
                                        .text_left()
                                        .child(number::compact(value)),
                                )
                        }),
                    )
                    .into_any_element(),
                None => div()
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.data.is_some() {
                        "此日期没有已记录数据，不能据此判断实际用量为零。"
                    } else {
                        "尚未读取日明细"
                    })
                    .into_any_element(),
            })
            .child(div().text_xs().text_color(cx.theme().muted_foreground).child(
                "总量优先使用已记录 total；缺失时使用输入 + 输出。缓存输入和推理不再重复累加。",
            ))
            .into_any_element()
    }
}

impl Render for UsageView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let loading = matches!(self.state, LoadState::Loading);
        div().id("usage-page").size_full().min_h_0().overflow_y_scroll()
            .bg(cx.theme().background).text_color(cx.theme().foreground)
            .child(div().w_full().max_w(rems(72.)).mx_auto().min_w_0().p_6().flex().flex_col().gap_6()
                .child(div().flex().items_center().gap_3()
                    .child(Button::new("return-to-sessions").ghost().small().label("返回会话")
                        .icon(Icon::new(IconName::ArrowLeft))
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(ReturnToSessions::new()))))
                    .child(div().flex_1().text_lg().font_weight(FontWeight::MEDIUM).child("统计"))
                    .child(Button::new("refresh-usage").ghost().small().label(if loading { "加载中…" } else { "刷新" })
                        .icon(Icon::new(IconName::RefreshCw)).disabled(loading)
                        .on_click(cx.listener(|this, _, window, cx| this.refresh(window, cx)))))
                .child(div().flex().flex_wrap().items_center().gap_3()
                    .child(div().flex().flex_col().gap_1().w_32().child("年份")
                        .child(self.year.to_string()))
                    .child(div().flex_1().min_w(rems(12.)).flex().flex_col().gap_1().child("项目 · cwd")
                        .child(Select::new(&self.project_select).small().accessibility_label("用量项目 cwd"))))
                .child(self.render_summary(cx))
                .when(loading, |view| view.child(div().text_color(cx.theme().muted_foreground).child("正在读取本地用量记录…")))
                .when_some(match &self.state { LoadState::Failed(error) => Some(error.clone()), _ => None }, |view, error| {
                    view.child(div().flex().flex_col().gap_2()
                        .child(div().text_color(cx.theme().danger).child(format!("无法读取用量：{error}")))
                        .child(Button::new("retry-usage").small().outline().label("重试")
                            .on_click(cx.listener(|this, _, window, cx| this.refresh(window, cx)))))
                })
                .when(matches!(self.state, LoadState::Ready) && self.days().is_none_or(|days| !days.keys().any(|date| date.year() == self.year && *date <= Zoned::now().date())), |view| {
                    view.child(div().text_color(cx.theme().muted_foreground).child("暂无数据 · 所选范围没有已记录用量"))
                })
                .child(self.render_heatmap(cx))
                .child(div().flex().flex_wrap().items_center().gap_2().text_xs().text_color(cx.theme().muted_foreground)
                    .child("少").children([0.25, 0.45, 0.7, 1.].into_iter().map(|opacity| div().size_3().rounded_sm().bg(cx.theme().primary.opacity(opacity))))
                    .child("多 · 按所选范围每日总量分级；浅色格为无记录或已记录零用量，未来日期留白"))
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child("方向键：左右移动一周，上下移动一天；Home / End：年初 / 最后可选日期；Tab：切换筛选与按钮。"))
                .child(self.render_detail(cx))
                .child(div().text_xs().text_color(cx.theme().muted_foreground)
                    .child("仅展示本地已记录数据，按本地时区归日。历史可能被清理，缺失记录不代表零用量。刷新可获取最新记录。")))
    }
}

#[derive(Debug, Error)]
enum LoadUsageError {
    #[error("无法确定 Aries 数据目录")]
    HomeUnavailable,
    #[error("Aries 数据库路径不是文件：{}", path.display())]
    NotFile { path: PathBuf },
    #[error("无法访问 Aries 数据库 {}", path.display())]
    Metadata { path: PathBuf, source: std::io::Error },
    #[error("无法启动用量查询运行时")]
    Runtime { source: std::io::Error },
    #[error("无法确定 {year} 年的用量日期范围")]
    YearRange { year: i16, source: jiff::Error },
    #[error("无法确定 {date} 的次日日期")]
    NextDay { date: Date, source: jiff::Error },
    #[error("无法确定 {first} 至 {next} 的用量时间范围")]
    TimestampRange { first: Date, next: Date, source: jiff::Error },
    #[error("无法连接本地 Aries 数据库 {}", path.display())]
    Connect { path: PathBuf, source: toasty::Error },
    #[error("无法查询 {year} 年的年度用量")]
    AnnualQuery { year: i16, source: toasty::Error },
    #[error("无法查询 {date} 的今日用量")]
    TodayQuery { date: Date, source: toasty::Error },
}

impl LoadUsageError {
    fn home_unavailable() -> Self {
        Self::HomeUnavailable
    }

    fn not_file(path: PathBuf) -> Self {
        Self::NotFile { path }
    }

    fn metadata(path: PathBuf, source: std::io::Error) -> Self {
        Self::Metadata { path, source }
    }

    fn runtime(source: std::io::Error) -> Self {
        Self::Runtime { source }
    }

    fn year_range(year: i16, source: jiff::Error) -> Self {
        Self::YearRange { year, source }
    }

    fn next_day(date: Date, source: jiff::Error) -> Self {
        Self::NextDay { date, source }
    }

    fn timestamp_range(first: Date, next: Date, source: jiff::Error) -> Self {
        Self::TimestampRange { first, next, source }
    }

    fn connect(path: PathBuf, source: toasty::Error) -> Self {
        Self::Connect { path, source }
    }

    fn annual_query(year: i16, source: toasty::Error) -> Self {
        Self::AnnualQuery { year, source }
    }

    fn today_query(date: Date, source: toasty::Error) -> Self {
        Self::TodayQuery { date, source }
    }
}

fn load_usage(year: i16) -> Result<UsageData, LoadUsageError> {
    let home = std::env::home_dir().ok_or_else(LoadUsageError::home_unavailable)?;
    let root = home.join(".local/share/aries");
    let path = root.join("aries.db");
    match fs::metadata(&path) {
        Ok(metadata) if metadata.is_file() => {},
        Ok(_) => return Err(LoadUsageError::not_file(path)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(UsageData::from_records(Vec::new(), Vec::new(), TimeZone::system()));
        },
        Err(source) => return Err(LoadUsageError::metadata(path, source)),
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(LoadUsageError::runtime)?;
    runtime.block_on(query_usage(&root, year))
}

async fn query_usage(root: impl AsRef<Path>, year: i16) -> Result<UsageData, LoadUsageError> {
    let root = root.as_ref();
    let timezone = TimeZone::system();
    let today = Timestamp::now().to_zoned(timezone.clone()).date();
    let first = Date::new(year, 1, 1).map_err(|source| LoadUsageError::year_range(year, source))?;
    let next = first
        .checked_add(Span::new().years(1))
        .map_err(|source| LoadUsageError::year_range(year, source))?;
    let (low, high) = timestamp_range(first, next, &timezone)?;
    let db = aries_persistence::connect(root)
        .await
        .map_err(|source| LoadUsageError::connect(root.join("aries.db"), source))?;
    let mut repository = TokenUsageRepository::new(db);
    let records = repository
        .find_by_created_at_between(low, high)
        .await
        .map_err(|source| LoadUsageError::annual_query(year, source))?;
    let tomorrow = today
        .checked_add(Span::new().days(1))
        .map_err(|source| LoadUsageError::next_day(today, source))?;
    let (low, high) = timestamp_range(today, tomorrow, &timezone)?;
    let today_records = repository
        .find_by_created_at_between(low, high)
        .await
        .map_err(|source| LoadUsageError::today_query(today, source))?;
    Ok(UsageData::from_records(records, today_records, timezone))
}

fn timestamp_range(
    first: Date,
    next: Date,
    timezone: &TimeZone,
) -> Result<(Timestamp, Timestamp), LoadUsageError> {
    let range = || -> Result<(Timestamp, Timestamp), jiff::Error> {
        let low = first.to_zoned(timezone.clone())?.timestamp();
        let high =
            next.to_zoned(timezone.clone())?.timestamp().checked_sub(Span::new().nanoseconds(1))?;
        Ok((low, high))
    };
    range().map_err(|source| LoadUsageError::timestamp_range(first, next, source))
}
