//! The activity heatmap between the filters and the table: GitHub's contributions graph, one
//! column per Sunday-start week and one row per weekday, square cells across the full width. It
//! covers 53 weeks with the period's week in the middle; the report's period is shown strong and
//! the days around it faded. A day's shade is its hours (with the filters); clicking a day moves the report
//! to the period that holds it.
use super::ReportsView;
use track_core::model::ReportRange;
use crate::ui::date_nav::{HasDateNav, NavEvent};
use crate::ui::theme::*;
use chrono::{Datelike, Duration, Local, NaiveDate};
use gpui::{Animation, AnimationExt};
use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px};

const WEEKDAYS: [&str; 7] = ["日", "月", "火", "水", "木", "金", "土"];
const GAP: f32 = 3.;
/// How faded the days around the period are.
const FADED: f32 = 0.35;
/// How long a cell takes to fade in or out when the period moves.
const FADE: std::time::Duration = std::time::Duration::from_millis(220);

/// 「12月」 when the week starting `week_first` holds the 1st of a month (where GitHub puts its
/// month labels).
pub fn month_label(week_first: NaiveDate) -> Option<String> {
    (0..7).map(|i| week_first + Duration::days(i)).find(|d| d.day() == 1).map(|d| format!("{}月", d.month()))
}

/// What clicking `day` moves the report to: its Sunday-start week, or its month (`[from, to)`).
pub fn pick_range(range: ReportRange, day: NaiveDate) -> (NaiveDate, NaiveDate) {
    match range {
        ReportRange::Week => track_core::dates::week_range(day),
        ReportRange::Month => track_core::dates::month_range(day),
    }
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
    /// The period shown strong (local dates). When it moves, the old one is kept to fade from.
    fn heat_period(&self) -> (NaiveDate, NaiveDate) {
        let (from, to) = track_core::reports::query_range(&self.current_query());
        (from.with_timezone(&Local).date_naive(), to.with_timezone(&Local).date_naive())
    }

    pub(super) fn track_heat_period(&mut self) {
        let now = self.heat_period();
        if self.heat_shown != Some(now) {
            self.heat_from = self.heat_shown;
            self.heat_shown = Some(now);
            self.heat_gen += 1;
        }
    }

    pub(super) fn render_heatmap(&self, window: &gpui::Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let days = self.heatmap.clone()?;
        if days.len() < 7 {
            return None;
        }
        let p = palette(cx);
        let (active_from, active_to) = self.heat_period();
        let before = self.heat_from;
        let generation = self.heat_gen;
        let today = Local::now().date_naive();
        let pick = self.heat_hover.map(|d| pick_range(self.range, d));
        let style = self.style(cx);
        let shade = |l: u8| match l {
            0 => p.hover,
            1 => with_alpha(p.heat, 0.3),
            2 => with_alpha(p.heat, 0.5),
            3 => with_alpha(p.heat, 0.75),
            _ => p.heat,
        };
        let weeks: Vec<&[(NaiveDate, i64)]> = days.chunks(7).collect();
        // Square cells across the full width: the page spans the window, so the side is the
        // window's width less the padding and the weekday labels, shared by the weeks.
        let n = weeks.len() as f32;
        let side = ((f32::from(window.viewport_size().width) - 24. - 18.) / n - GAP).floor().max(6.);
        let label = |text: &'static str| div().w(px(18.)).flex_none().text_size(px(10.)).text_color(p.text_faint).child(text);
        // Month names over the weeks that hold a 1st (they may run over the next columns).
        // Each cell's hit area takes half the gap on every side, so the gaps click too.
        let slot = side + GAP;
        let months = div().flex().h(px(14.)).child(label("")).children(weeks.iter().map(|w| {
            div().w(px(slot)).flex_none().pl(px(GAP / 2.)).text_size(px(10.)).text_color(p.text_faint).whitespace_nowrap().children(month_label(w[0].0))
        }));
        // One row per weekday; the hit areas touch, so the rows line up with the labels.
        let rows = (0..7).map(|wd| {
            div().flex().items_center().child(label(WEEKDAYS[wd])).children(weeks.iter().map(|w| {
                let (d, minutes) = w[wd];
                let active = d >= active_from && d < active_to;
                let picked = pick.is_some_and(|(f, t)| d >= f && d < t);
                let was = before.map(|(f, t)| d >= f && d < t);
                let id = format!("heat-{d}");
                let sel = id.clone();
                let tip = format!("{}（{}） {}", d.format("%-m/%-d"), WEEKDAYS[wd], crate::core::time::format_duration(minutes, style));
                // The report's period is strong; the weeks around it are faded. Under the pointer,
                // the days a click would show are not faded.
                let target = if active || picked { 1. } else { FADED };
                let square = div()
                    .debug_selector(move || sel.clone())
                    .size(px(side))
                    .rounded(RADIUS_SM)
                    .bg(shade(level(minutes)))
                    .when(d == today, |c| c.border_1().border_color(p.text_muted));
                let square = match was {
                    // Moved into or out of the period: fade over FADE.
                    Some(was) if was != active && !picked => {
                        let start = if was { 1. } else { FADED };
                        square
                            .with_animation(SharedString::from(format!("heat-fade-{d}-{generation}")), Animation::new(FADE).with_easing(gpui::ease_out_quint()), move |c, t| {
                                c.opacity(start + (target - start) * t)
                            })
                            .into_any_element()
                    }
                    _ => square.opacity(target).into_any_element(),
                };
                div()
                    .id(SharedString::from(format!("heat-hit-{d}")))
                    .size(px(slot))
                    .flex_none()
                    .p(px(GAP / 2.))
                    .cursor_pointer()
                    .child(square)
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        if *hovered {
                            this.heat_hover = Some(d);
                        } else if this.heat_hover == Some(d) {
                            this.heat_hover = None;
                        }
                        cx.notify();
                    }))
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
