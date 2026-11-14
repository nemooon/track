//! The activity heatmap between the filters and the table: GitHub's contributions graph, one
//! column per Sunday-start week and one row per weekday, square cells across the full width. It
//! covers 53 weeks with the period's week in the middle; the report's period is shown strong and
//! the days around it faded. A day's shade is its hours (with the filters); clicking a day moves the report
//! to the period that holds it.
use super::ReportsView;
use crate::ui::date_nav::{HasDateNav, NavEvent};
use crate::ui::theme::*;
use chrono::{Datelike, Duration, Local, NaiveDate};
use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px};

const WEEKDAYS: [&str; 7] = ["日", "月", "火", "水", "木", "金", "土"];
const GAP: f32 = 3.;

/// 「12月」 when the week starting `week_first` holds the 1st of a month (where GitHub puts its
/// month labels).
pub fn month_label(week_first: NaiveDate) -> Option<String> {
    (0..7).map(|i| week_first + Duration::days(i)).find(|d| d.day() == 1).map(|d| format!("{}月", d.month()))
}

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
    pub(super) fn render_heatmap(&self, window: &gpui::Window, cx: &mut Context<Self>) -> Option<AnyElement> {
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
        let weeks: Vec<&[(NaiveDate, i64)]> = days.chunks(7).collect();
        // Square cells across the full width: the page spans the window, so the side is the
        // window's width less the padding and the weekday labels, shared by the weeks.
        let n = weeks.len() as f32;
        let side = ((f32::from(window.viewport_size().width) - 24. - 18. - GAP * n) / n).floor().max(6.);
        let label = |text: &'static str| div().w(px(18.)).flex_none().text_size(px(10.)).text_color(p.text_faint).child(text);
        // Month names over the weeks that hold a 1st (they may run over the next columns).
        let months = div().flex().gap(px(GAP)).h(px(14.)).child(label("")).children(weeks.iter().map(|w| {
            div().w(px(side)).flex_none().text_size(px(10.)).text_color(p.text_faint).whitespace_nowrap().children(month_label(w[0].0))
        }));
        // One row per weekday; a cell's height follows its width, so rows line up with the labels.
        let rows = (0..7).map(|wd| {
            div().flex().items_center().gap(px(GAP)).child(label(WEEKDAYS[wd])).children(weeks.iter().map(|w| {
                let (d, minutes) = w[wd];
                let active = d >= active_from && d < active_to;
                let id = format!("heat-{d}");
                let sel = id.clone();
                let tip = format!("{}（{}） {}", d.format("%-m/%-d"), WEEKDAYS[wd], crate::core::time::format_duration(minutes, style));
                div()
                    .id(SharedString::from(id))
                    .debug_selector(move || sel.clone())
                    .size(px(side))
                    .flex_none()
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
                .flex_col()
                .flex_shrink_0()
                .gap(px(GAP))
                .px_3()
                .py_3()
                .border_b_1()
                .border_color(p.border)
                .child(months)
                .children(rows)
                .into_any_element(),
        )
    }
}
