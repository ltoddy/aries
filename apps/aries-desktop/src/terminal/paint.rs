use alacritty_terminal::index::Point as GridPoint;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Rgb};
use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    Bounds, Context, Font, FontFeatures, Hsla, Pixels, TextAlign, TextRun, Window, fill, point, px,
    rgb, size,
};

use crate::terminal::TerminalView;

const ANSI: [u32; 16] = [
    0x000000, 0xcd0000, 0x00cd00, 0xcdcd00, 0x0000ee, 0xcd00cd, 0x00cdcd, 0xe5e5e5, 0x7f7f7f,
    0xff0000, 0x00ff00, 0xffff00, 0x5c5cff, 0xff00ff, 0x00ffff, 0xffffff,
];

pub fn indexed_rgb(index: usize) -> Rgb {
    let value = match index {
        0..=15 => ANSI[index],
        16..=231 => {
            let index = index - 16;
            let component = |value: usize| if value == 0 { 0 } else { 55 + value * 40 };
            ((component(index / 36) << 16) | (component(index / 6 % 6) << 8) | component(index % 6))
                as u32
        },
        232..=255 => {
            let gray = (8 + (index - 232) * 10) as u32;
            (gray << 16) | (gray << 8) | gray
        },
        _ => 0,
    };
    Rgb { r: (value >> 16) as u8, g: (value >> 8) as u8, b: value as u8 }
}

fn color(value: Color, colors: &Colors, foreground: Hsla, background: Hsla, bold: bool) -> Hsla {
    let index = match value {
        Color::Spec(value) => {
            return rgb((u32::from(value.r) << 16)
                | (u32::from(value.g) << 8)
                | u32::from(value.b))
            .into();
        },
        Color::Indexed(value) => usize::from(value),
        Color::Named(value) => {
            if bold {
                value.to_bright() as usize
            } else {
                value as usize
            }
        },
    };
    if let Some(value) = colors[index] {
        return rgb((u32::from(value.r) << 16) | (u32::from(value.g) << 8) | u32::from(value.b))
            .into();
    }
    match index {
        256 | 267 | 268 => foreground,
        257 => background,
        258 => foreground,
        259..=266 => {
            color(Color::Indexed((index - 259) as u8), colors, foreground, background, false)
        },
        _ => {
            let value = indexed_rgb(index);
            rgb((u32::from(value.r) << 16) | (u32::from(value.g) << 8) | u32::from(value.b)).into()
        },
    }
}

pub fn cell_width(window: &Window, font_size: Pixels) -> Pixels {
    window
        .text_system()
        .shape_line(
            "M".into(),
            font_size,
            &[TextRun { len: 1, font: gpui_kit::font("Menlo"), ..TextRun::default() }],
            None,
        )
        .width()
        .max(px(1.))
}

#[derive(PartialEq)]
struct RunStyle {
    font: Font,
    foreground: Hsla,
    background: Hsla,
    flags: Flags,
}

impl RunStyle {
    fn new(font: Font, foreground: Hsla, background: Hsla, flags: Flags) -> Self {
        Self { font, foreground, background, flags }
    }
}

struct Run {
    row: i32,
    end_column: usize,
    bounds: Bounds<Pixels>,
    text: String,
    style: RunStyle,
    force_width: Option<Pixels>,
}

impl Run {
    fn new(
        row: i32,
        end_column: usize,
        bounds: Bounds<Pixels>,
        text: String,
        style: RunStyle,
        force_width: Option<Pixels>,
    ) -> Self {
        Self { row, end_column, bounds, text, style, force_width }
    }

    fn paint(self, font_size: Pixels, window: &mut Window, cx: &mut Context<TerminalView>) {
        if !self.text.is_empty() {
            let run = TextRun {
                len: self.text.len(),
                font: self.style.font,
                color: self.style.foreground,
                ..TextRun::default()
            };
            let line = window.text_system().shape_line(
                self.text.into(),
                font_size,
                &[run],
                self.force_width,
            );
            let _ = line.paint(
                self.bounds.origin,
                self.bounds.size.height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        }
        if self.style.flags.intersects(Flags::ALL_UNDERLINES) {
            window.paint_quad(fill(
                Bounds::new(
                    self.bounds.origin + point(px(0.), self.bounds.size.height - px(2.)),
                    size(self.bounds.size.width, px(1.)),
                ),
                self.style.foreground,
            ));
        }
        if self.style.flags.contains(Flags::STRIKEOUT) {
            window.paint_quad(fill(
                Bounds::new(
                    self.bounds.origin + point(px(0.), self.bounds.size.height * 0.55),
                    size(self.bounds.size.width, px(1.)),
                ),
                self.style.foreground,
            ));
        }
    }
}

pub fn paint(
    view: &mut TerminalView,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut Context<TerminalView>,
) {
    let Some(session) = &view.session else {
        return;
    };
    let (cells, cursor, display_offset, colors, selection) = {
        let Ok(terminal) = session.terminal() else {
            return;
        };
        let content = terminal.renderable_content();
        let cells: Vec<_> =
            content.display_iter.map(|cell| (cell.point, cell.cell.clone())).collect();
        (cells, content.cursor, content.display_offset, *content.colors, content.selection)
    };
    let foreground = color(
        Color::Named(NamedColor::Foreground),
        &colors,
        cx.theme().foreground,
        cx.theme().background,
        false,
    );
    let background = color(
        Color::Named(NamedColor::Background),
        &colors,
        cx.theme().foreground,
        cx.theme().background,
        false,
    );
    let font_size = window.rem_size() * 0.875;
    let cell_size = view.cell_size;
    let focused = view.focus_handle.is_focused(window);
    let cursor_row = cursor.point.line.0 + display_offset as i32;
    view.cursor_bounds = Bounds::new(
        bounds.origin
            + point(
                cell_size.width * cursor.point.column.0 as f32,
                cell_size.height * cursor_row.max(0) as f32,
            ),
        cell_size,
    );
    let cursor_color =
        color(Color::Named(NamedColor::Cursor), &colors, foreground, background, false);
    let mut ascii_font = gpui_kit::font("Menlo");
    ascii_font.features = FontFeatures(std::sync::Arc::new(
        ["calt", "liga", "clig", "dlig"].map(|tag| (tag.to_owned(), 0)).to_vec(),
    ));
    window.with_content_mask(Some(gpui_kit::ContentMask { bounds }), |window| {
        if background != cx.theme().background {
            window.paint_quad(fill(bounds, background));
        }
        for (position, cell) in &cells {
            let row = position.line.0 + display_offset as i32;
            if row < 0 || row as usize >= view.dimensions.lines {
                continue;
            }
            let mut bg = color(cell.bg, &colors, foreground, background, false);
            if cell.flags.contains(Flags::INVERSE) {
                bg = color(
                    cell.fg,
                    &colors,
                    foreground,
                    background,
                    cell.flags.contains(Flags::BOLD),
                );
            }
            if bg != background {
                let width = if cell.flags.contains(Flags::WIDE_CHAR) {
                    cell_size.width * 2.
                } else {
                    cell_size.width
                };
                window.paint_quad(fill(
                    Bounds::new(
                        bounds.origin
                            + point(
                                cell_size.width * position.column.0 as f32,
                                cell_size.height * row as f32,
                            ),
                        size(width, cell_size.height),
                    ),
                    bg,
                ));
            }
        }
        if display_offset == 0
            && focused
            && cursor.shape == CursorShape::Block
            && cursor_row >= 0
            && (cursor_row as usize) < view.dimensions.lines
            && let Some((_, cell)) = cells.iter().find(|(position, _)| *position == cursor.point)
            && !cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
        {
            let width = if cell.flags.contains(Flags::WIDE_CHAR) {
                cell_size.width * 2.
            } else {
                cell_size.width
            };
            window.paint_quad(fill(
                Bounds::new(view.cursor_bounds.origin, size(width, cell_size.height)),
                cursor_color,
            ));
        }
        let mut pending: Option<Run> = None;
        for (position, cell) in cells {
            let row = position.line.0 + display_offset as i32;
            if row < 0 || row as usize >= view.dimensions.lines {
                continue;
            }
            if cell.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                if let Some(run) = pending.take() {
                    run.paint(font_size, window, cx);
                }
                continue;
            }
            let width = if cell.flags.contains(Flags::WIDE_CHAR) {
                cell_size.width * 2.
            } else {
                cell_size.width
            };
            let cell_bounds = Bounds::new(
                bounds.origin
                    + point(
                        cell_size.width * position.column.0 as f32,
                        cell_size.height * row as f32,
                    ),
                size(width, cell_size.height),
            );
            let mut fg =
                color(cell.fg, &colors, foreground, background, cell.flags.contains(Flags::BOLD));
            let mut bg = color(cell.bg, &colors, foreground, background, false);
            if cell.flags.contains(Flags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }
            let cursor_here = position == cursor.point
                && display_offset == 0
                && cursor.shape != CursorShape::Hidden;
            if cursor_here && focused && cursor.shape == CursorShape::Block {
                bg = cursor_color;
                fg = background;
            }
            if cell.flags.contains(Flags::DIM) {
                fg.a *= 0.65;
            }
            if selection.is_some_and(|selection| {
                selection.contains(position)
                    || (cell.flags.contains(Flags::WIDE_CHAR)
                        && selection.contains(GridPoint::new(position.line, position.column + 1)))
            }) {
                fg = cx.theme().primary_foreground;
                bg = cx.theme().primary;
                window.paint_quad(fill(cell_bounds, bg));
            }
            let extra = cell.zerowidth().filter(|extra| !extra.is_empty());
            let ascii = cell.c.is_ascii_graphic()
                && extra.is_none()
                && !cell.flags.contains(Flags::WIDE_CHAR);
            let mut font = if ascii { ascii_font.clone() } else { gpui_kit::font("Menlo") };
            if cell.flags.contains(Flags::BOLD) {
                font = font.bold();
            }
            if cell.flags.contains(Flags::ITALIC) {
                font = font.italic();
            }
            let style = RunStyle::new(font, fg, bg, cell.flags);
            let merge = !cursor_here
                && !cell.flags.contains(Flags::HIDDEN)
                && ascii
                && pending.as_ref().is_some_and(|run| {
                    run.row == row
                        && run.end_column == position.column.0
                        && run.style == style
                        && run.force_width.is_some()
                });
            if merge {
                if let Some(run) = pending.as_mut() {
                    run.text.push(cell.c);
                    run.end_column += 1;
                    run.bounds.size.width = cell_size.width * (run.text.len() as f32);
                }
            } else {
                if let Some(run) = pending.take() {
                    run.paint(font_size, window, cx);
                }
                if !cell.flags.contains(Flags::HIDDEN) {
                    let mut text = String::new();
                    if cell.c != ' ' || extra.is_some() {
                        text.push(cell.c);
                        if let Some(extra) = extra {
                            text.extend(extra);
                        }
                    }
                    let run = Run::new(
                        row,
                        position.column.0 + 1,
                        cell_bounds,
                        text,
                        style,
                        ascii.then_some(cell_size.width),
                    );
                    if ascii && !cursor_here {
                        pending = Some(run);
                    } else {
                        run.paint(font_size, window, cx);
                    }
                }
            }
            if cursor_here {
                let geometry = match (focused, cursor.shape) {
                    (true, CursorShape::Beam) => {
                        Some(Bounds::new(cell_bounds.origin, size(px(2.), cell_size.height)))
                    },
                    (true, CursorShape::Underline) => Some(Bounds::new(
                        cell_bounds.origin + point(px(0.), cell_size.height - px(2.)),
                        size(width, px(2.)),
                    )),
                    (false, _) => Some(Bounds::new(
                        cell_bounds.origin + point(px(0.), cell_size.height - px(1.)),
                        size(width, px(1.)),
                    )),
                    _ => None,
                };
                if let Some(geometry) = geometry {
                    window.paint_quad(fill(geometry, cursor_color));
                }
            }
        }
        if let Some(run) = pending.take() {
            run.paint(font_size, window, cx);
        }
        if !view.preedit.is_empty() {
            let run = TextRun {
                len: view.preedit.len(),
                font: gpui_kit::font("Menlo"),
                color: foreground,
                ..TextRun::default()
            };
            let line = window.text_system().shape_line(
                view.preedit.clone().into(),
                font_size,
                &[run],
                None,
            );
            let preedit_bounds =
                Bounds::new(view.cursor_bounds.origin, size(line.width(), cell_size.height));
            window.paint_quad(fill(preedit_bounds, background));
            let _ = line.paint(
                preedit_bounds.origin,
                cell_size.height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
            window.paint_quad(fill(
                Bounds::new(
                    preedit_bounds.origin + point(px(0.), cell_size.height - px(1.)),
                    size(line.width(), px(1.)),
                ),
                foreground,
            ));
        }
    });
}
