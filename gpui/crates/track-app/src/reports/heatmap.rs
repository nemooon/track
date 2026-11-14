//! The activity heatmap between the filters and the table: GitHub's contributions graph, one
//! column per Sunday-start week and one row per weekday, across the full width. It covers five
//! weeks around the week (or the month's weeks); the report's period is shown strong and the days
//! around it faded. A day's shade is its hours (with the filters); clicking a day moves the report
//! to the period that holds it.
use super::ReportsView;
use crate::ui::date_nav::{HasDateNav, NavEvent};
use crate::ui::theme::*;
use chrono::{Datelike, Local};
use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px};

const WEEKDAYS: [&str; 7] = ["日", "月", "火", "水", "木", "金", "土"];
const CELL_HEIGHT: f32 = 14.;

/// The shade of a day: 0 for nothing, then under 2h, under 4h, under 6h, and 6h or more.
pub fn level(minutes: i64) -> u8 {
    match minutes {
        m if m <= 0 => 0,
        m if m < 120 => 1,
        m if m < 240 => 2,
        m if m < 360 => 3,
        _ => 4,
    }
}

impl ReportsView {
    pub(super) fn render_heatmap(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let days = self.heatmap.clone()?;
        if days.len() < 7 {
            return None;
        }
        let p = palette(cx);
        let (from, to) = track_core::reports::query_range(&self.current_query());
        let (active_from, active_to) = (from.with_timezone(&Local).date_naive(), to.with_timezone(&Local).date_naive());
        let today = Local::now().date_naive();
        let style = self.style(cx);
        let shade = |l: u8| match l {
            0 => p.hover,
            1 => with_alpha(p.accent, 0.3),
            2 => with_alpha(p.accent, 0.5),
            3 => with_alpha(p.accent, 0.75),
            _ => p.accent,
        };
        let label_col = div()
            .flex()
            .flex_col()
            .gap(px(3.))
            .w(px(18.))
            .flex_none()
            .pt(px(18.))
            .children(WEEKDAYS.iter().map(|w| div().h(px(CELL_HEIGHT)).flex().items_center().text_size(px(10.)).text_color(p.text_faint).child(*w)));
        let weeks = days.chunks(7).map(|week| {
            let first = week[0].0;
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(div().h(px(15.)).text_size(px(10.)).text_color(p.text_faint).child(format!("{}/{}", first.month(), first.day())))
                .children(week.iter().map(|&(d, minutes)| {
                    let active = d >= active_from && d < active_to;
                    let id = format!("heat-{d}");
                    let sel = id.clone();
                    let tip = format!("{}（{}） {}", d.format("%-m/%-d"), WEEKDAYS[d.weekday().num_days_from_sunday() as usize], crate::core::time::format_duration(minutes, style));
                    div()
                        .id(SharedString::from(id))
                        .debug_selector(move || sel.clone())
                        .h(px(CELL_HEIGHT))
                        .rounded(RADIUS_SM)
                        .cursor_pointer()
                        .bg(shade(level(minutes)))
                        // The report's period is strong; the weeks around it are faded.
                        .when(!active, |c| c.opacity(0.35))
                        .when(d == today, |c| c.border_1().border_color(p.text_muted))
                        .tooltip(move |window, cx| gpui_component::tooltip::Tooltip::new(tip.clone()).build(window, cx))
                        .on_click(cx.listener(move |this, _, window, cx| this.on_nav(NavEvent::Pick(d), window, cx)))
                }))
        });
        Some(
            div()
                .debug_selector(|| "report-heatmap".into())
                .flex()
                .flex_shrink_0()
                .gap(px(3.))
                .px_3()
                .py_3()
                .border_b_1()
                .border_color(p.border)
                .child(label_col)
                .children(weeks)
                .into_any_element(),
        )
    }
}
