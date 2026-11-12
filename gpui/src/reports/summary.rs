//! 集計サマリー: donut (recharts PieChart look-alike), legend, collapsed strip.
use crate::api::{GroupBy, ReportRow};
use crate::core::report::view::{display_label, percent, row_color};
use crate::core::time::format_duration_report;
use crate::ui::theme::*;
use gpui::{
    App, Bounds, Div, FontWeight, ParentElement, PathBuilder, Pixels, Point, Styled, Window, canvas, div, linear_color_stop,
    linear_gradient, point, prelude::FluentBuilder, px,
};
use std::f32::consts::{PI, TAU};

/// Slice angles in radians (start, end) with 2° padding when n > 1. Angle 0 is 3 o'clock and
/// angles grow counter-clockwise, like recharts' default startAngle=0 / endAngle=360.
pub fn slice_angles(values: &[i64]) -> Vec<(f32, f32)> {
    let total: i64 = values.iter().sum();
    if total <= 0 {
        return vec![];
    }
    let pad = if values.len() > 1 { 2f32.to_radians() } else { 0. };
    let usable = TAU - pad * values.len() as f32;
    let mut a = 0.;
    values
        .iter()
        .map(|v| {
            let sweep = usable * (*v as f32 / total as f32);
            let s = (a, a + sweep);
            a += sweep + pad;
            s
        })
        .collect()
}

/// Which slice is under `p` (relative to the donut centre), if inside the ring.
pub fn hit_slice(angles: &[(f32, f32)], dx: f32, dy: f32, inner: f32, outer: f32) -> Option<usize> {
    let r = (dx * dx + dy * dy).sqrt();
    if r < inner || r > outer {
        return None;
    }
    // screen y grows downward; recharts angles grow counter-clockwise from 3 o'clock
    let mut ang = (-dy).atan2(dx);
    if ang < 0. {
        ang += TAU;
    }
    angles.iter().position(|(s, e)| ang >= *s && ang <= *e)
}

fn paint_donut(bounds: Bounds<Pixels>, rows: &[(i64, gpui::Rgba)], window: &mut Window, _: &mut App) {
    let size = f32::from(bounds.size.width.min(bounds.size.height));
    let c = bounds.center();
    let (outer, inner) = (size / 2. * 0.95, size / 2. * 0.62);
    let angles = slice_angles(&rows.iter().map(|r| r.0).collect::<Vec<_>>());
    let pt = |r: f32, a: f32| -> Point<Pixels> { point(c.x + px(r * a.cos()), c.y - px(r * a.sin())) };
    for ((s, e), (_, color)) in angles.iter().zip(rows) {
        let steps = (((e - s) / (PI / 90.)).ceil() as usize).max(2);
        let mut b = PathBuilder::fill();
        b.move_to(pt(outer, *s));
        for i in 1..=steps {
            b.line_to(pt(outer, s + (e - s) * i as f32 / steps as f32));
        }
        for i in (0..=steps).rev() {
            b.line_to(pt(inner, s + (e - s) * i as f32 / steps as f32));
        }
        b.close();
        if let Ok(path) = b.build() {
            // linearGradient x1=0 y1=0 → x2=1 y2=1, opacity .85 → 1
            let bg = linear_gradient(135., linear_color_stop(with_alpha(*color, 0.85), 0.), linear_color_stop(with_alpha(*color, 1.), 1.));
            window.paint_path(path, bg);
        }
    }
}

pub struct SummaryProps<'a> {
    pub rows: &'a [ReportRow],
    pub total: i64,
    pub group_by: GroupBy,
    pub hovered: Option<usize>,
}

pub fn colors(rows: &[ReportRow]) -> Vec<gpui::Rgba> {
    rows.iter().enumerate().map(|(i, r)| hex(&row_color(r.color.as_deref(), i))).collect()
}

pub fn donut(p: &SummaryProps) -> Div {
    let cols = colors(p.rows);
    let data: Vec<(i64, gpui::Rgba)> = p.rows.iter().map(|r| r.total_minutes).zip(cols.iter().copied()).collect();
    let tooltip = p.hovered.and_then(|i| p.rows.get(i).map(|r| (r, cols[i])));
    let group_by = p.group_by;
    div()
        .relative()
        .size(px(224.))
        .flex_shrink_0()
        .child(canvas(|_, _, _| (), move |b, _, w, cx| paint_donut(b, &data, w, cx)).absolute().size_full())
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .child(div().text_size(px(10.)).text_color(n400()).child("TOTAL"))
                .child(div().mt(px(2.)).text_2xl().font_weight(FontWeight::SEMIBOLD).text_color(n900()).child(format_duration_report(p.total))),
        )
        .when_some(tooltip, |d, (r, color)| {
            d.child(
                div()
                    .absolute()
                    .top_2()
                    .left_2()
                    .rounded_lg()
                    .border_1()
                    .border_color(n200())
                    .bg(white_a(0.9))
                    .px_3()
                    .py_2()
                    .text_xs()
                    .shadow_lg()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(n900())
                            .when(r.color.is_some(), |d| d.child(div().size(px(8.)).rounded_full().bg(color)))
                            .child(display_label(group_by, &r.label)),
                    )
                    .child(div().mt(px(2.)).text_color(n600()).child(format_duration_report(r.total_minutes))),
            )
        })
}

pub fn legend(p: &SummaryProps) -> Div {
    let cols = colors(p.rows);
    div().w_full().min_w_0().flex_1().flex().flex_col().gap(px(10.)).text_sm().children(p.rows.iter().enumerate().map(|(i, r)| {
        let pct = if p.total > 0 { r.total_minutes as f32 / p.total as f32 } else { 0. };
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .min_w_0()
                    .items_center()
                    .gap_2()
                    .child(div().size(px(10.)).flex_shrink_0().rounded_full().bg(cols[i]))
                    .child(div().min_w_0().flex_1().truncate().text_color(n700()).child(display_label(p.group_by, &r.label)))
                    .child(div().flex_shrink_0().text_xs().text_color(n700()).child(format_duration_report(r.total_minutes)))
                    .child(div().w(px(36.)).flex_shrink_0().flex().justify_end().text_xs().text_color(n400()).child(format!("{}%", percent(r.total_minutes, p.total)))),
            )
            .child(
                div().ml(px(18.)).h(px(6.)).overflow_hidden().rounded_full().bg(n100()).child(
                    div()
                        .h_full()
                        .w(gpui::relative(pct))
                        .rounded_full()
                        .bg(linear_gradient(90., linear_color_stop(with_alpha(cols[i], 0.7), 0.), linear_color_stop(with_alpha(cols[i], 1.), 1.))),
                ),
            )
    }))
}

/// Collapsed: a thin stacked bar sized by minutes (vertical when the summary column is on the left).
pub fn collapsed_strip(rows: &[ReportRow], vertical: bool) -> Div {
    let cols = colors(rows);
    div()
        .flex()
        .gap(px(1.))
        .bg(white())
        .map(|d| if vertical { d.flex_col().w(px(8.)).h_full() } else { d.h(px(8.)).w_full() })
        // flexGrow proportional to minutes
        .children(rows.iter().enumerate().map(|(i, r)| div().flex_basis(px(0.)).flex_grow(r.total_minutes.max(1) as f32).min_w(px(1.)).min_h(px(1.)).bg(cols[i])))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_cover_circle_minus_padding() {
        let a = slice_angles(&[30, 90]);
        assert_eq!(a.len(), 2);
        let total: f32 = a.iter().map(|(s, e)| e - s).sum();
        assert!((total - (TAU - 4f32.to_radians())).abs() < 1e-4);
        assert!((a[1].1 - a[0].1 - (2f32.to_radians() + (a[1].1 - a[1].0))).abs() < 1e-4);
        assert_eq!(slice_angles(&[0, 0]), vec![]);
        let one = slice_angles(&[5]);
        assert!((one[0].1 - TAU).abs() < 1e-5);
    }

    #[test]
    fn hit_slice_by_angle_and_ring() {
        let a = slice_angles(&[50, 50]);
        assert_eq!(hit_slice(&a, 1., -70., 60., 100.), Some(0)); // top: first slice (from 3 o'clock, counter-clockwise)
        assert_eq!(hit_slice(&a, 1., 70., 60., 100.), Some(1)); // bottom: second slice
        assert_eq!(hit_slice(&a, 10., 0., 60., 100.), None); // inside the hole
    }
}
