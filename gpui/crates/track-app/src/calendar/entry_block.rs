//! EntryBlock.tsx: one time entry inside a day column.
use track_core::model::TimeEntry;
use crate::core::geometry::minutes_to_y;
use crate::core::time::format_duration_calendar;
use crate::ui::theme::*;
use chrono::Local;
use gpui::{
    BoxShadow, Div, FontWeight, InteractiveElement, ParentElement, Stateful, Styled, div, point,
    prelude::FluentBuilder, px,
};

pub struct BlockGeometry {
    pub start_min: i64,
    pub end_min: i64,
    pub lane: usize,
    pub lane_count: usize,
}

pub fn break_label(break_min: i64) -> String {
    if break_min <= 0 {
        String::new()
    } else {
        format!("（休憩 {} 込み）", format_duration_calendar(break_min))
    }
}

/// Outer absolutely-positioned lane slot; the block itself sits at left 2px / right 4px
/// inside it, which equals `left: lane*100%/n + 2px; width: 100%/n - 6px`.
pub fn lane_slot(g: &BlockGeometry, hour_px: f32) -> Div {
    let top = minutes_to_y(g.start_min, hour_px) + 2.;
    let height = (minutes_to_y(g.end_min - g.start_min, hour_px) - 5.).max(14.);
    div()
        .absolute()
        .top(px(top))
        .h(px(height))
        .left(gpui::relative(g.lane as f32 / g.lane_count as f32))
        .w(gpui::relative(1. / g.lane_count as f32))
}

pub fn render_block(p: Palette, entry: &TimeEntry, selected: bool, ghost: bool, hovered_id: &str) -> Stateful<Div> {
    let accent = hex(entry.project.as_ref().map(|p| p.color.as_str()).unwrap_or(NO_PROJECT));
    let hovered = hovered_id == entry.id;
    // A pale wash of the project colour with a slightly stronger rim (Zed-like, no heavy shadow).
    let (border, bg) = if selected {
        (mix(accent, 0.6, p.bg), mix(accent, 0.18, p.bg))
    } else if hovered {
        (mix(accent, 0.45, p.bg), mix(accent, 0.15, p.bg))
    } else {
        (mix(accent, 0.3, p.bg), mix(accent, 0.12, p.bg))
    };
    let text = p.text;
    let start = entry.start.with_timezone(&Local);
    let end = entry.end.with_timezone(&Local);
    let gross = (entry.end - entry.start).num_minutes();
    let dur = format!("{}{}", format_duration_calendar((gross - entry.break_minutes).max(0)), break_label(entry.break_minutes));
    let project_name = entry.project.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| "プロジェクトなし".into());
    let title = entry.title.clone().filter(|t| !t.is_empty());
    let mut shadows = vec![];
    if selected {
        shadows.push(BoxShadow { color: with_alpha(accent, 0.22), offset: point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(2.), inset: false });
    }
    let id_owned = entry.id.clone();
    div()
        .id(gpui::SharedString::from(format!("entry-{id_owned}")))
        .debug_selector(move || format!("entry-{id_owned}"))
        .absolute()
        .top_0()
        .bottom_0()
        .left(px(2.))
        .right(px(4.))
        .overflow_hidden()
        .rounded(px(5.))
        .border_1()
        .border_color(border)
        .bg(bg)
        .text_color(text)
        .px_2()
        .py_1()
        .text_xs()
        .shadow(shadows)
        .when(ghost, |d| d.opacity(0.6))
        .cursor(if ghost { gpui::CursorStyle::ClosedHand } else { gpui::CursorStyle::OpenHand })
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .h_full()
                .overflow_hidden()
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(6.))
                        .child(div().min_w_0().flex_1().truncate().font_weight(FontWeight::SEMIBOLD).line_height(px(15.)).child(title.clone().unwrap_or(project_name.clone())))
                        .child(
                            div()
                                .flex_shrink_0()
                                .rounded_sm()
                                .px_1()
                                .text_size(px(9.))
                                .font_weight(FontWeight::MEDIUM)
                                .line_height(px(12.))
                                .font_family(MONO)
                                .text_color(p.text_muted)
                                .bg(with_alpha(p.bg, 0.6))
                                .child(format!("{}–{}", start.format("%H:%M"), end.format("%H:%M"))),
                        ),
                )
                .when(title.is_some(), |d| {
                    d.child(
                        div()
                            .flex()
                            .items_baseline()
                            .gap(px(6.))
                            .line_height(px(14.))
                            .child(div().min_w_0().flex_1().truncate().text_size(px(11.)).font_weight(FontWeight::MEDIUM).opacity(0.65).child(project_name.clone()))
                            .child(div().ml_auto().flex_shrink_0().text_size(px(10.)).opacity(0.55).child(dur.clone())),
                    )
                })
                .when(title.is_none() || !entry.tags.is_empty(), |d| {
                    d.child(
                        div()
                            .mt(px(2.))
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_x(px(6.))
                            .gap_y(px(2.))
                            .text_size(px(10.))
                            .line_height(px(13.))
                            .children(entry.tags.iter().map(|t| {
                                div()
                                    .flex()
                                    .min_w_0()
                                    .items_center()
                                    .gap_1()
                                    .opacity(0.65)
                                    .child(div().size(px(6.)).flex_shrink_0().rounded_full().bg(hex(&t.tag.color)))
                                    .child(div().truncate().child(t.tag.name.clone()))
                            }))
                            .when(title.is_none(), |d| d.child(div().ml_auto().opacity(0.55).child(dur))),
                    )
                }),
        )
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn break_label_formats() {
        assert_eq!(break_label(0), "");
        assert_eq!(break_label(45), "（休憩 45m 込み）");
        assert_eq!(break_label(90), "（休憩 1h 30m 込み）");
    }
}
