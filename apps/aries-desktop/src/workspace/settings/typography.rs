use gpui_kit::component::text::{TextView, TextViewStyle};
use gpui_kit::{Pixels, Rems, StyleRefinement, Styled, Window, relative, rems};

pub const BASE_FONT_SIZE: f32 = 16.;
pub const BODY_FONT_SIZE: f32 = 15.;
pub const CODE_FONT_SIZE: f32 = 14.;

pub fn body_size() -> Rems {
    rems(BODY_FONT_SIZE / BASE_FONT_SIZE)
}

pub fn code_size() -> Rems {
    rems(CODE_FONT_SIZE / BASE_FONT_SIZE)
}

pub fn code_size_in_window(window: &Window) -> Pixels {
    code_size().to_pixels(window.rem_size())
}

pub fn style_markdown(view: TextView) -> TextView {
    view.style(
        TextViewStyle::default()
            .paragraph_gap(rems(0.5))
            .code_block(StyleRefinement::default().text_size(code_size())),
    )
    .inline_code_style(StyleRefinement::default().text_size(code_size()))
    .text_size(body_size())
    .line_height(relative(1.5))
}
