use crate::app::AppState;
use crate::api::ReportRange;
use crate::ui::{controls, date_nav::{self, DateNav, HasDateNav, NavEvent, NavKind}};
use chrono::{Datelike, Duration, Local, NaiveDate};
use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};

pub struct ReportsView {
    state: Entity<AppState>,
    pub anchor: NaiveDate,
    range: ReportRange,
    nav: DateNav,
}

impl ReportsView {
    pub fn new(state: Entity<AppState>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state, anchor: Local::now().date_naive(), range: ReportRange::Week, nav: DateNav::default() }
    }
}

pub fn shift_month(d: NaiveDate, delta: i32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 + delta;
    let (y, m) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
    let last = crate::core::time::month_range(NaiveDate::from_ymd_opt(y, m, 1).unwrap()).1 - Duration::days(1);
    NaiveDate::from_ymd_opt(y, m, d.day().min(last.day())).unwrap()
}

impl HasDateNav for ReportsView {
    fn date_nav(&mut self) -> &mut DateNav {
        &mut self.nav
    }

    fn on_nav(&mut self, ev: NavEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.anchor = match (ev, self.range) {
            (NavEvent::Prev, ReportRange::Week) => self.anchor - Duration::days(7),
            (NavEvent::Next, ReportRange::Week) => self.anchor + Duration::days(7),
            (NavEvent::Prev, ReportRange::Month) => shift_month(self.anchor, -1),
            (NavEvent::Next, ReportRange::Month) => shift_month(self.anchor, 1),
            (NavEvent::Today, _) => Local::now().date_naive(),
            (NavEvent::Pick(d), _) => d,
        };
        cx.notify();
    }
}

impl Render for ReportsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let kind = if self.range == ReportRange::Week { NavKind::Week } else { NavKind::Month };
        let _ = &self.state;
        div().size_full().flex().flex_col().child(controls::toolbar().child(date_nav::render(self.nav, self.anchor, kind, cx)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_month_clamps_day() {
        let d = |y, m, day| NaiveDate::from_ymd_opt(y, m, day).unwrap();
        assert_eq!(shift_month(d(2026, 3, 31), -1), d(2026, 2, 28));
        assert_eq!(shift_month(d(2026, 12, 15), 1), d(2027, 1, 15));
    }
}
