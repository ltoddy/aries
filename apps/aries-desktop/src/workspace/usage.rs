use std::collections::BTreeMap;

use aries_proto::DailyUsage;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;
use jiff::civil::Date;
use jiff::{Span, Zoned};
use thiserror::Error;

use crate::ipc::{Client, Ipc, IpcError};
use crate::workspace::number;

struct UsageData {
    days: BTreeMap<Date, u64>,
}

impl UsageData {
    fn from_daily_usages(
        usages: impl IntoIterator<Item = DailyUsage>,
    ) -> Result<Self, LoadUsageError> {
        let mut days: BTreeMap<Date, u64> = BTreeMap::new();
        for usage in usages {
            let (Some(date), Some(tokens)) = (usage.date, usage.token_usage) else {
                continue;
            };
            let date = Date::new(date.year as i16, date.month as i8, date.day as i8)
                .map_err(LoadUsageError::invalid_date)?;
            let tokens = if tokens.total_tokens == 0 {
                tokens.input_tokens.saturating_add(tokens.output_tokens)
            } else {
                tokens.total_tokens
            };
            let total = days.entry(date).or_default();
            *total = total.saturating_add(tokens);
        }
        Ok(Self { days })
    }
}

enum LoadState {
    Idle,
    Loading,
    Ready,
    Failed(String),
}

impl LoadState {
    fn idle() -> Self {
        Self::Idle
    }

    fn loading() -> Self {
        Self::Loading
    }

    fn ready() -> Self {
        Self::Ready
    }

    fn failed(error: String) -> Self {
        Self::Failed(error)
    }
}

pub struct UsageView {
    data: Option<UsageData>,
    state: LoadState,
    today: Date,
    selected: Date,
    focus: FocusHandle,
    scroll: ScrollHandle,
    _task: Option<Task<()>>,
}

impl UsageView {
    pub fn new(_: &mut Window, cx: &mut Context<Self>) -> Self {
        let today = Zoned::now().date();
        Self {
            data: None,
            state: LoadState::idle(),
            today,
            selected: today,
            focus: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            _task: None,
        }
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        self.scroll_to_selected();
        if matches!(self.state, LoadState::Idle) || self.today != Zoned::now().date() {
            self.refresh(window, cx);
        }
    }

    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self._task = None;
        let today = Zoned::now().date();
        self.today = today;
        self.selected = self.selected.clamp(self.first_day(), self.last_day());
        self.scroll_to_selected();
        self.data = None;
        self.state = LoadState::loading();
        let client = cx.global::<Ipc>().client();
        let background = cx.background_executor().spawn(async move { load_usage(&client) });
        self._task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = background.await;
            let _ = this.update_in(cx, |this, _, cx| {
                match result {
                    Ok(data) => {
                        this.data = Some(data);
                        this.state = LoadState::ready();
                    },
                    Err(error) => {
                        crate::error::log("无法读取用量", &error);
                        this.state = LoadState::failed(crate::error::details(&error));
                    },
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn days(&self) -> Option<&BTreeMap<Date, u64>> {
        self.data.as_ref().map(|data| &data.days)
    }

    fn first_day(&self) -> Date {
        self.today.saturating_sub(Span::new().years(1)).saturating_add(Span::new().days(1))
    }

    fn last_day(&self) -> Date {
        self.today
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
        let week = ((self.selected - first).get_days()
            + i32::from(first.weekday().to_sunday_zero_offset()))
            / 7;
        self.scroll.scroll_to_item(week as usize);
    }

    fn render_heatmap(&self, cx: &mut Context<Self>) -> AnyElement {
        let first = self.first_day();
        let last = self.last_day();
        let offset = i32::from(first.weekday().to_sunday_zero_offset());
        let count = (last - first).get_days() + 1;
        let weeks = (offset + count + 6) / 7;
        let days = self.days();
        let maximum = days.into_iter().flat_map(|days| days.values()).copied().max().unwrap_or(0);
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
                    .aria_label(format!("过去一年 token 用量热力图：{first} 至 {last}"))
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
                                (date.day() == 1 || date == first)
                                    .then(|| format!("{}月", date.month()))
                            })
                            .unwrap_or_default();
                        div()
                            .id(SharedString::from(format!(
                                "usage-week-{}",
                                first.saturating_add(Span::new().days(start))
                            )))
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
                                let date = date.filter(|_| (0..count).contains(&ordinal));
                                let Some(date) = date else {
                                    return div().size_4().into_any_element();
                                };
                                let usage = days.and_then(|days| days.get(&date));
                                let color = usage
                                    .filter(|usage| **usage > 0)
                                    .map(|usage| {
                                        let level = if maximum == 0 {
                                            1.
                                        } else {
                                            *usage as f32 / maximum as f32
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
                                        format!("{date}：{} tokens", number::compact(*usage))
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
        let total = self
            .days()
            .map(|days| days.values().fold(0u64, |total, tokens| total.saturating_add(*tokens)));
        let value = total
            .map(|total| format!("{} tokens", number::compact(total)))
            .unwrap_or_else(|| "—".into());
        let exact =
            total.map(|total| format!("{total} tokens")).unwrap_or_else(|| "尚未读取".into());
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div().text_xs().text_color(cx.theme().muted_foreground).child("过去一年累计用量"),
            )
            .child(
                div()
                    .id("usage-total")
                    .aria_label(format!("过去一年累计用量：{exact}"))
                    .tooltip(move |window, cx| Tooltip::new(exact.clone()).build(window, cx))
                    .text_lg()
                    .font_weight(FontWeight::MEDIUM)
                    .child(value),
            )
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
                Some(tokens) => {
                    let exact = format!("{tokens} tokens");
                    div()
                        .id("usage-day-total")
                        .aria_label(format!("{}：{exact}", self.selected))
                        .tooltip(move |window, cx| Tooltip::new(exact.clone()).build(window, cx))
                        .child(format!("{} tokens", number::compact(*tokens)))
                        .into_any_element()
                },
                None => div()
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.data.is_some() { "无记录" } else { "尚未读取" })
                    .into_any_element(),
            })
            .into_any_element()
    }
}

impl Render for UsageView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let loading = matches!(self.state, LoadState::Loading);
        let first = self.first_day();
        let last = self.last_day();
        div().id("usage-page").size_full().min_h_0().overflow_y_scroll()
            .bg(cx.theme().background).text_color(cx.theme().foreground)
            .child(div().w_full().max_w(rems(72.)).mx_auto().min_w_0().p_6().flex().flex_col().gap_6()
                .child(div().flex().items_center().gap_3()
                    .child(div().flex_1().min_w_0().text_sm().text_color(cx.theme().muted_foreground)
                        .child(format!("过去一年 · {first} 至 {last}")))
                    .child(Button::new("refresh-usage").ghost().small().label(if loading { "加载中…" } else { "刷新" })
                        .icon(Icon::new(IconName::RefreshCw)).disabled(loading)
                        .on_click(cx.listener(|this, _, window, cx| this.refresh(window, cx)))))
                .child(self.render_summary(cx))
                .when(loading, |view| view.child(div().text_color(cx.theme().muted_foreground).child("正在读取本地用量记录…")))
                .when_some(match &self.state { LoadState::Failed(error) => Some(error.clone()), _ => None }, |view, error| {
                    view.child(div().flex().flex_col().gap_2()
                        .child(div().text_color(cx.theme().danger).child(format!("无法读取用量：{error}")))
                        .child(Button::new("retry-usage").small().outline().label("重试")
                            .on_click(cx.listener(|this, _, window, cx| this.refresh(window, cx)))))
                })
                .when(matches!(self.state, LoadState::Ready) && self.days().is_none_or(BTreeMap::is_empty), |view| {
                    view.child(div().text_color(cx.theme().muted_foreground).child("过去一年暂无用量记录"))
                })
                .child(self.render_heatmap(cx))
                .child(div().flex().flex_wrap().items_center().gap_2().text_xs().text_color(cx.theme().muted_foreground)
                    .child("少").children([0.25, 0.45, 0.7, 1.].into_iter().map(|opacity| div().size_3().rounded_sm().bg(cx.theme().primary.opacity(opacity))))
                    .child("多 · 按每日 token 用量分级；浅色格为无记录或零用量"))
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child("方向键：左右移动一周，上下移动一天；Home / End：首日 / 今日；Tab：切换按钮与热力图。"))
                .child(self.render_detail(cx))
                .child(div().text_xs().text_color(cx.theme().muted_foreground)
                    .child("按本地时区汇总已记录用量；缺失记录不代表零用量。")))
    }
}

#[derive(Debug, Error)]
enum LoadUsageError {
    #[error("无法查询本地用量")]
    Query { source: IpcError },
    #[error("本地用量服务返回了无效日期")]
    InvalidDate { source: jiff::Error },
}

impl LoadUsageError {
    fn query(source: IpcError) -> Self {
        Self::Query { source }
    }

    fn invalid_date(source: jiff::Error) -> Self {
        Self::InvalidDate { source }
    }
}

fn load_usage(client: &Client) -> Result<UsageData, LoadUsageError> {
    let response = client.list_token_usages().map_err(LoadUsageError::query)?;
    UsageData::from_daily_usages(response.daily_usages)
}
