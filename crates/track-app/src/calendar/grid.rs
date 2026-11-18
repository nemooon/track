//! Painted layers of a day column: hatched buffers, 30-minute dashes, hour lines.
use crate::ui::theme::*;
use gpui::{App, Bounds, Hsla, PathBuilder, Pixels, Window, fill, point, px, size};

/// The text color's alpha that draws a line as dark as `border_subtle` on the plain background.
pub const LINE_TINT: f32 = 0.08;
/// The same for the column borders (as dark as `border`).
pub const BORDER_TINT: f32 = 0.15;

/// How much of the shade a dark theme lays over grid lines and dividers (relative to the background's).
pub const DARK_MARK: f32 = 0.5;

/// A grid line's tint over off hours whose background has alpha `a`: darker in proportion.
pub fn off_hours_line_alpha(a: f32) -> f32 {
    LINE_TINT * (1. + a)
}

fn is_light(p: &Palette) -> bool {
    p.bg.l > 0.5
}

/// The off-hours background at depth `a` (0 = work hours, `OFF_HOURS_DARKEST` = midnight).
pub fn off_hours_bg(p: &Palette, a: f32) -> Hsla {
    mix(p.off_hours_shade, a, p.bg).into()
}

/// A grid line over off hours at depth `a`. A light theme tints it with the text color, deeper with
/// the background; a dark one shades the usual line, so it darkens too but stays lighter than the
/// background.
pub fn off_hours_line(p: &Palette, a: f32) -> Hsla {
    if is_light(p) { mix(p.text, off_hours_line_alpha(a), off_hours_bg(p, a)).into() } else { mix(p.off_hours_shade, a * DARK_MARK, p.border_subtle).into() }
}

/// The column divider over off hours at depth `a`, darkening the same way as the lines.
pub fn off_hours_divider(p: &Palette, a: f32) -> Hsla {
    if is_light(p) { mix(p.text, BORDER_TINT * a, p.border).into() } else { mix(p.off_hours_shade, a * DARK_MARK, p.border).into() }
}

fn line_color(cx: &App) -> gpui::Hsla {
    palette(cx).border_subtle
}

/// The buffers beyond 0:00 and 24:00: the off-hours gradient's darkest shade, with faint stripes.
pub fn paint_hatch(bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let p = palette(cx);
    window.paint_quad(fill(bounds, off_hours_bg(&p, crate::calendar::OFF_HOURS_DARKEST)));
    paint_stripes(bounds, with_alpha(p.text, LINE_TINT), window);
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
    let p = palette(cx);
    let line = |x: f32, y: f32, width: f32, minute: i64, window: &mut Window| {
        let r = Bounds::new(point(px(x0 + x), px(y)), size(px(width), px(1.)));
        let a = crate::calendar::off_hours_alpha(minute, hours, band);
        // Over off hours the line darkens with the background, so it keeps standing out.
        let color = if a > 0. { off_hours_line(&p, a) } else { line_color(cx) };
        window.paint_quad(fill(r, color));
    };
    // 0:00, mirroring the 24:00 line at the bottom.
    line(0., y0, w, 0, window);
    for hour in 0..24 {
        let half = hour * 60 + 30;
        let y = y0 + hour as f32 * hour_px + hour_px / 2. - 1.;
        let mut x = 0.;
        while x < w {
            line(x, y, 4f32.min(w - x), half, window);
            x += 7.;
        }
        // Hour lines sit just above the hour; 24:00 takes the body's last row, against the buffer.
        let y = y0 + (hour + 1) as f32 * hour_px - if hour == 23 { 0. } else { 1. };
        line(0., y, w, (hour + 1) * 60, window);
    }
}
