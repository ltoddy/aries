use std::time::Duration;

use gpui_kit::{AppContext, Context};

use crate::error;
use crate::workspace::files::file_tree::FileTree;
use crate::workspace::git::{GitWatch, WatchError};

const WATCH_DELAY: Duration = Duration::from_millis(300);

impl FileTree {
    pub fn start_watch(&mut self, cx: &mut Context<Self>) {
        self.watch_task = None;
        self.watch_error = None;
        let root = self.root.clone();
        self.watch_task = Some(cx.spawn(async move |this, cx| {
            let (sender, receiver) = async_channel::bounded(1);
            let executor = cx.background_executor().clone();
            let directory = root.clone();
            let watcher = cx
                .background_spawn(async move { GitWatch::start_tree(&directory, sender, executor) })
                .await;
            let watcher = match watcher {
                Ok(watcher) => watcher,
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        if this.root == root {
                            this.report_watch_error(error, cx);
                        }
                    });
                    return;
                },
            };
            if !this
                .update(cx, |this, cx| {
                    if this.root != root {
                        return false;
                    }
                    this.load(cx);
                    true
                })
                .is_ok_and(|current| current)
            {
                return;
            }
            while receiver.recv().await.is_ok() {
                cx.background_executor().timer(WATCH_DELAY).await;
                while receiver.try_recv().is_ok() {}
                let (error, poisoned) = match watcher.take_error() {
                    Ok(error) => (error, false),
                    Err(error) => (Some(error), true),
                };
                let _ = watcher.take_paths();
                if !this
                    .update(cx, |this, cx| {
                        if this.root != root {
                            return false;
                        }
                        this.load(cx);
                        if let Some(error) = error {
                            this.report_watch_error(error, cx);
                        }
                        true
                    })
                    .is_ok_and(|current| current)
                {
                    return;
                }
                if poisoned {
                    break;
                }
            }
        }));
    }

    fn report_watch_error(&mut self, source: WatchError, cx: &mut Context<Self>) {
        let message = format!("文件树自动刷新不可用：{}。刷新可重试。", error::details(&source));
        if self.watch_error.as_ref() != Some(&message) {
            error::log("文件树自动刷新不可用", &source);
            self.watch_error = Some(message);
            cx.notify();
        }
    }
}
