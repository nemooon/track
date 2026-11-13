//! Painted layers of a day column: hatched buffers, 30-minute dashes, hour lines.
use crate::ui::theme::*;
use gpui::{App, Bounds, PathBuilder, Pixels, Window, fill, point, px, size};

fn line_color(cx: &App) -> gpui::Hsla {
    palette(cx).border_subtle
}

/// `repeating-linear-gradient(45deg, #e5e5e5 0 4px, #f5f5f5 4px 10px)`
pub fn paint_hatch(bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let p = palette(cx);
    window.paint_quad(fill(bounds, p.off_hours));
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
                window.paint_path(path, p.border_subtle);
            }
            x += period;
        }
    });
}

/// Dashed 30-minute ticks (4px on / 3px off) and solid hour lines, as in WeekCalendar.tsx.
pub fn paint_grid_lines(bounds: Bounds<Pixels>, hour_px: f32, window: &mut Window, cx: &mut App) {
    let (x0, y0) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    let w = f32::from(bounds.size.width);
    for hour in 0..24 {
        let y = y0 + hour as f32 * hour_px + hour_px / 2. - 1.;
        let mut x = 0.;
        while x < w {
            let seg = 4f32.min(w - x);
            window.paint_quad(fill(Bounds::new(point(px(x0 + x), px(y)), size(px(seg), px(1.))), line_color(cx)));
            x += 7.;
        }
        let y = y0 + (hour + 1) as f32 * hour_px - 1.;
        window.paint_quad(fill(Bounds::new(point(px(x0), px(y)), size(px(w), px(1.))), line_color(cx)));
    }
}
