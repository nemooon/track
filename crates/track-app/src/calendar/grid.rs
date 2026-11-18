//! Painted layers of a day column: hatched buffers, 30-minute dashes, hour lines.
use crate::ui::theme::*;
use gpui::{App, Bounds, PathBuilder, Pixels, Window, fill, point, px, size};

/// The text color's alpha that draws a line as dark as `border_subtle` on the plain background.
pub const LINE_TINT: f32 = 0.08;
/// The same for the column borders (as dark as `border`).
pub const BORDER_TINT: f32 = 0.15;

/// A grid line's tint over off hours whose background has alpha `a`: darker in proportion.
pub fn off_hours_line_alpha(a: f32) -> f32 {
    LINE_TINT * (1. + a)
}

fn line_color(cx: &App) -> gpui::Hsla {
    palette(cx).border_subtle
}

/// `repeating-linear-gradient(45deg, #e5e5e5 0 4px, #f5f5f5 4px 10px)`
pub fn paint_hatch(bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let p = palette(cx);
    window.paint_quad(fill(bounds, p.off_hours));
    paint_stripes(bounds, p.border_subtle, window);
}

/// 45° stripes, 4px on and 6px off, clipped to `bounds`.
pub fn paint_stripes(bounds: Bounds<Pixels>, color: gpui::Hsla, window: &mut Window) {
    let period = 10. * std::f32::consts::SQRT_2;
    let stripe = 4. * std::f32::consts::SQRT_2;
    let h = f32::from(bounds.size.height);
    let w = f32::from(bounds.size.width);
    let (x0, y0) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
        let mut x = -h;
        while x < w {
            let mut b = PathBuilder::fill();
            b.move_to(point(px(x0 + x), px(y0 + h)));
            b.line_to(point(px(x0 + x + stripe), px(y0 + h)));
            b.line_to(point(px(x0 + x + stripe + h), px(y0)));
            b.line_to(point(px(x0 + x + h), px(y0)));
            b.close();
            if let Ok(path) = b.build() {
                window.paint_path(path, color);
            }
            x += period;
        }
    });
}

/// Dashed 30-minute ticks (4px on / 3px off) and solid hour lines. Off hours (outside `band`)
/// darken the lines as much as the background under them.
pub fn paint_grid_lines(bounds: Bounds<Pixels>, hour_px: f32, hours: (i64, i64), band: Option<(i64, i64)>, window: &mut Window, cx: &mut App) {
    let (x0, y0) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    let w = f32::from(bounds.size.width);
    let text = palette(cx).text;
    let line = |x: f32, y: f32, width: f32, minute: i64, window: &mut Window| {
        let r = Bounds::new(point(px(x0 + x), px(y)), size(px(width), px(1.)));
        let a = crate::calendar::off_hours_alpha(minute, hours, band);
        // Over the off-hours background the line is a tint of the text color that deepens with it,
        // so it keeps standing out instead of fading into the gray.
        let color = if a > 0. { with_alpha(text, off_hours_line_alpha(a)) } else { line_color(cx) };
        window.paint_quad(fill(r, color));
    };
    for hour in 0..24 {
        let half = hour * 60 + 30;
        let y = y0 + hour as f32 * hour_px + hour_px / 2. - 1.;
        let mut x = 0.;
        while x < w {
            line(x, y, 4f32.min(w - x), half, window);
            x += 7.;
        }
        let y = y0 + (hour + 1) as f32 * hour_px - 1.;
        line(0., y, w, (hour + 1) * 60, window);
    }
}
