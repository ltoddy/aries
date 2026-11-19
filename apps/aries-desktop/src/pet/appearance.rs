use std::f32::consts::TAU;

use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    Bounds, Hsla, IntoElement, PathBuilder, Styled, Window, canvas, fill, point, px, size,
};

pub fn sheep(phase: f32, working: bool, finished: bool) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, cx| {
            let scale = (bounds.size.width / 128.).min(bounds.size.height / 104.);
            if scale <= px(0.) {
                return;
            }
            let origin = bounds.origin
                + point(
                    (bounds.size.width - scale * 128.) / 2.,
                    (bounds.size.height - scale * 104.) / 2.,
                );
            let jump = if finished { (phase * TAU * 1.2).sin().max(0.).powi(2) * 4. } else { 0. };
            let step = if working && !finished { (phase * TAU * 1.5).sin() * 2. } else { 0. };
            let theme = cx.theme();
            let (wool, ink) = if theme.background.l >= theme.foreground.l {
                (theme.background, theme.foreground)
            } else {
                (theme.foreground, theme.background)
            };
            let edge = theme.border.blend(ink.opacity(0.7));
            let face = wool.blend(theme.border.opacity(0.35));
            let at = |x: f32, y: f32| origin + point(scale * x, scale * (y - jump));
            let draw = |window: &mut Window, x, y, w: f32, h: f32, color: Hsla| {
                window.paint_quad(
                    fill(Bounds::new(at(x, y), size(scale * w, scale * h)), color)
                        .corner_radii(scale * (w.min(h) / 2.)),
                );
            };
            window.paint_quad(
                fill(
                    Bounds::new(
                        origin + point(scale * (42. + jump), scale * 94.),
                        size(scale * (44. - jump * 2.), scale * 5.),
                    ),
                    ink.opacity(0.14),
                )
                .corner_radii(scale * 2.5),
            );
            for (x, direction) in [(43., 1.), (55., -1.), (67., 1.), (79., -1.)] {
                let lift = step * direction;
                draw(window, x, 78. + lift, 9., 16., face);
                draw(window, x - 1., 87. + lift, 11., 8., wool);
                draw(window, x, 88. + lift, 9., 6., ink);
            }
            let tufts = [
                (42., 42., 13.),
                (45., 31., 11.),
                (60., 27., 12.),
                (75., 29., 12.),
                (86., 39., 13.),
                (91., 53., 12.),
                (85., 66., 13.),
                (72., 75., 12.),
                (56., 76., 12.),
                (42., 69., 13.),
                (34., 55., 12.),
            ];
            for (extra, color) in [(2.4, wool), (1.2, edge), (0., wool)] {
                for (x, y, radius) in tufts {
                    let r = radius + extra;
                    draw(window, x - r, y - r, r * 2., r * 2., color);
                }
            }
            draw(window, 38., 32., 54., 49., wool);
            for x in [29., 78.] {
                let y = if finished { 36. } else { 39. };
                draw(window, x - 1., y - 1., 23., 14., edge);
                draw(window, x, y, 21., 12., face);
                draw(window, x + 5., y + 4., 11., 4., theme.primary.opacity(0.4));
            }
            draw(window, 45., 31., 38., 37., edge);
            draw(window, 46.5, 32.5, 35., 34., face);
            for (x, y, r) in [(53., 33., 8.), (64., 30., 9.), (75., 33., 7.)] {
                draw(window, x - r, y - r, r * 2., r * 2., wool);
            }
            draw(window, 41., 65., 46., 11., edge);
            draw(window, 42.5, 66.5, 43., 8., theme.primary);
            draw(window, 76., 71., 11., 19., edge);
            draw(window, 77.5, 72.5, 8., 16., theme.primary);
            draw(window, 78.5, 84., 6., 2., theme.primary_foreground.opacity(0.6));
            let blinking = !working && !finished && phase.rem_euclid(4.2) > 3.95;
            for x in [54., 70.] {
                let height = if blinking { 1.5 } else { 5.5 };
                draw(window, x, 49. - height / 2., 4., height, ink);
                if !blinking {
                    draw(window, x + 0.8, 46.8, 1.2, 1.2, wool);
                }
            }
            draw(window, 49., 55., 7., 3., theme.primary.opacity(0.22));
            draw(window, 73., 55., 7., 3., theme.primary.opacity(0.22));
            draw(window, 61.5, 53., 5., 3., ink);
            let mut smile = PathBuilder::stroke(scale * 1.3);
            smile.move_to(at(64., 55.));
            smile.line_to(at(64., 58.));
            smile.move_to(at(58., 58.));
            smile.curve_to(at(70., 58.), at(64., if finished { 65. } else { 62. }));
            if let Ok(path) = smile.build() {
                window.paint_path(path, ink);
            }
        },
    )
    .w_full()
    .h_full()
}
