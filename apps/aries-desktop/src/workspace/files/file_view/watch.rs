use std::time::Duration;

use gpui_kit::{AppContext, Context, Window};
use thiserror::Error;

use crate::error;
use crate::workspace::files::file_view::FileView;
use crate::workspace::git::{GitWatch, WatchError};

const WATCH_DELAY: Duration = Duration::from_millis(300);

#[derive(Debug, Error)]
#[error("文件自动刷新不可用，请重新读取以重启监听")]
struct WatchFileError {
    source: WatchError,
}

impl WatchFileError {
    fn new(source: WatchError) -> Self {
        Self { source }
    }
}

impl FileView {
    pub fn start_watch(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.watch_task = None;
        self.watch_error = None;
        self.watch_identity = None;
        let Some(target) = self.target.clone() else {
            return;
        };
        let revision = self.revision;
        self.watch_task = Some(cx.spawn_in(window, async move |this, cx| {
            let (sender, receiver) = async_channel::bounded(1);
            let executor = cx.background_executor().clone();
            let watcher = cx
                .background_spawn(async move {
                    GitWatch::start_file(target.directory(), target.path(), sender, executor)
                })
                .await;
            let (watcher, identity) = match watcher {
                Ok(watcher) => watcher,
                Err(error) => {
                    let _ = this.update_in(cx, |this, window, cx| {
                        if this.revision == revision {
                            this.report_watch_error(error, window, cx);
                        }
                    });
                    return;
                },
            };
            if !this
                .update_in(cx, |this, window, cx| {
                    if this.revision != revision {
                        return false;
                    }
                    this.watch_identity = Some(identity);
                    this.refresh_from_disk(window, cx);
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
                    .update_in(cx, |this, window, cx| {
                        if this.revision != revision {
                            return false;
                        }
                        if let Some(error) = error {
                            this.report_watch_error(error, window, cx);
                        }
                        this.refresh_from_disk(window, cx);
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

    fn report_watch_error(
        &mut self,
        source: WatchError,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let failure = WatchFileError::new(source);
        let message = error::details(&failure);
        if self.watch_error.as_ref() != Some(&message) {
            error::report("文件自动刷新不可用", &failure, window, cx);
            self.watch_error = Some(message);
            cx.notify();
        }
    }
}
