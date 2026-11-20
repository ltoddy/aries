use std::time::{Duration, Instant};

use gpui_kit::component::ActiveTheme;
use gpui_kit::prelude::*;
use gpui_kit::{Context, IntoElement, Render, SharedString, Task, Window, div};

const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);

pub struct FpsCounter {
    sampled_at: Instant,
    frames_drawn: u64,
    label: SharedString,
    _task: Task<()>,
}

impl FpsCounter {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let frames_drawn = Self::frames_drawn(window);
        let sampled_at = Instant::now();
        let task = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(SAMPLE_INTERVAL).await;
                if this.update_in(cx, |this, window, cx| this.sample(window, cx)).is_err() {
                    break;
                }
            }
        });
        Self { sampled_at, frames_drawn, label: "— FPS".into(), _task: task }
    }

    fn frames_drawn(window: &Window) -> u64 {
        window
            .frame_duration_snapshot()
            .by_refresh_interval
            .iter()
            .map(|frames| frames.frames_drawn)
            .sum()
    }

    fn sample(&mut self, window: &Window, cx: &mut Context<Self>) {
        let frames_drawn = Self::frames_drawn(window);
        let now = Instant::now();
        let elapsed = now.duration_since(self.sampled_at).as_secs_f64();
        let fps = frames_drawn.saturating_sub(self.frames_drawn) as f64 / elapsed;
        self.sampled_at = now;
        self.frames_drawn = frames_drawn;
        let label: SharedString = format!("{fps:.0} FPS").into();
        if self.label != label {
            self.label = label;
            cx.notify();
        }
    }
}

impl Render for FpsCounter {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_end()
            .text_xs()
            .font_family(cx.theme().mono_font_family.clone())
            .text_color(cx.theme().muted_foreground)
            .child(self.label.clone())
    }
}
