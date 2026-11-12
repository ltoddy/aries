use std::error::Error;

use gpui_kit::component::WindowExt;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::notification::Notification;
use gpui_kit::{Anchor, App, ClipboardItem, SharedString, Window};

pub fn init() {
    let _ = tracing_subscriber::fmt().with_target(false).with_writer(std::io::stderr).try_init();
}

pub fn details(error: &(dyn Error + 'static)) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(error) = source {
        let cause = error.to_string();
        if !message.ends_with(&cause) {
            message.push_str(": ");
            message.push_str(&cause);
        }
        source = error.source();
    }
    message
}

pub fn log(title: &str, error: &(dyn Error + 'static)) {
    tracing::error!(operation = title, error = %details(error), "操作失败");
}

pub fn report(
    title: impl Into<SharedString>,
    error: &(dyn Error + 'static),
    window: &mut Window,
    cx: &mut App,
) {
    let title = title.into();
    log(title.as_ref(), error);
    let details = format!("{title}：{}", details(error));
    let key = SharedString::from(details.clone());
    window.push_notification(
        Notification::error(error.to_string())
            .title(title)
            .id1::<Notification>(key)
            .placement(Anchor::TopRight)
            .action(move |_, _, _| {
                let details = details.clone();
                Button::new("copy-error-details").ghost().label("复制详情").on_click(
                    move |_, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(details.clone()));
                    },
                )
            }),
        cx,
    );
}
