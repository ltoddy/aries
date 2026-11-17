use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::files::OpenFile;

pub fn single_line(text: impl AsRef<str>) -> String {
    text.as_ref().split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn file_button(
    id: impl Into<ElementId>,
    label: String,
    target: OpenFile,
    session: WeakEntity<SessionView>,
) -> Button {
    Button::new(id)
        .ghost()
        .small()
        .max_w_full()
        .min_w_0()
        .icon(IconName::FileText)
        .accessibility_label(format!("打开文件：{label}"))
        .label(label)
        .on_click(move |_, window, cx| {
            let _ = session.update(cx, |this, cx| this.open_file(target.clone(), window, cx));
        })
}

pub fn link_handler(
    session: WeakEntity<SessionView>,
) -> impl Fn(&SharedString, &ClickEvent, &mut Window, &mut App) + Send + Sync + 'static {
    move |url, event, window, cx| {
        let activate = match event {
            ClickEvent::Mouse(click) => {
                matches!(click.up.button, MouseButton::Left | MouseButton::Middle)
            },
            ClickEvent::Keyboard(_) => true,
            ClickEvent::Touch(click) => !click.long_press,
        };
        if !activate {
            return;
        }
        if !url.starts_with('#')
            && let Some(session) = session.upgrade()
            && let Some(target) = OpenFile::from_location(
                session.read(cx).execution().directory().to_owned(),
                url.as_ref(),
            )
        {
            session.update(cx, |this, cx| this.open_file(target, window, cx));
        } else {
            cx.open_url(url);
        }
    }
}
