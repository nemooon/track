use crate::app::AppState;
use crate::core::time::{DayCount, effective_day_count, step_days};
use crate::ui::{controls, date_nav::{self, DateNav, HasDateNav, NavEvent, NavKind}};
use chrono::{Duration, Local, NaiveDate};
use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};

pub struct CalendarView {
    state: Entity<AppState>,
    pub anchor: NaiveDate,
    day_count: DayCount,
    nav: DateNav,
}

impl CalendarView {
    pub fn new(state: Entity<AppState>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self { state, anchor: Local::now().date_naive(), day_count: DayCount::Workdays, nav: DateNav::default() }
    }

    pub fn zoom(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.state.update(cx, |s, _| {
            s.update_persist(|p| p.zoom_index = (p.zoom_index as i32 + delta).clamp(0, 2) as usize)
        });
        cx.notify();
    }

    pub fn nav_open(&self) -> bool {
        self.nav.open
    }

    fn effective(&self, window: &Window) -> DayCount {
        effective_day_count(self.day_count, f32::from(window.viewport_size().width))
    }
}

impl HasDateNav for CalendarView {
    fn date_nav(&mut self) -> &mut DateNav {
        &mut self.nav
    }

    fn on_nav(&mut self, ev: NavEvent, window: &mut Window, cx: &mut Context<Self>) {
        let step = step_days(self.effective(window));
        self.anchor = match ev {
            NavEvent::Prev => self.anchor - Duration::days(step),
            NavEvent::Next => self.anchor + Duration::days(step),
            NavEvent::Today => Local::now().date_naive(),
            NavEvent::Pick(d) => d,
        };
        cx.notify();
    }
}

impl Render for CalendarView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let eff = self.effective(window);
        let kind = match eff {
            DayCount::One => NavKind::Day,
            DayCount::Three => NavKind::Days(3),
            _ => NavKind::Week,
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(controls::toolbar().child(date_nav::render(self.nav, self.anchor, kind, cx)))
            .child(div().flex_1().child(format!("zoom {}", self.state.read(cx).persist.zoom_index)))
    }
}
