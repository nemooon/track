mod entry_block;
mod grid;

use crate::api::TimeEntry;
use crate::app::AppState;
use crate::core::geometry::{BUFFER_PX, GUTTER_PX, ZOOM_LEVELS, at_minutes, minutes_to_y, y_to_minutes};
use crate::core::holidays::holiday_name;
use crate::core::layout::{day_total_minutes, entries_for_day, entry_span_minutes, layout_blocks};
use crate::core::time::{DAY_MINUTES, DayCount, effective_day_count, format_duration_calendar, step_days, visible_days, weekday_ja};
use crate::ui::{controls, date_nav::{self, DateNav, HasDateNav, NavEvent, NavKind}, theme::*};
use chrono::{Datelike, Duration, Local, NaiveDate, Timelike};
use entry_block::{BlockGeometry, lane_slot, render_block};
use gpui::{
    AnyElement, Bounds, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Pixels, Render,
    ScrollHandle, StatefulInteractiveElement, Styled, Task, Window, canvas, div, point, prelude::FluentBuilder, px,
};
use std::cell::RefCell;
use std::rc::Rc;

pub struct CalendarView {
    state: Entity<AppState>,
    pub anchor: NaiveDate,
    pub day_count: DayCount,
    nav: DateNav,
    pub entries: Vec<TimeEntry>,
    requested: Option<(NaiveDate, NaiveDate)>,
    load_task: Option<Task<()>>,
    pub selected: Option<String>,
    hovered_entry: String,
    hover: Option<(usize, i64)>,
    column_bounds: Rc<RefCell<Vec<Bounds<Pixels>>>>,
    scroll: ScrollHandle,
    initial_scroll_done: bool,
    _ticker: Task<()>,
}

impl CalendarView {
    pub fn new(state: Entity<AppState>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        // Re-render every minute for the current-time line.
        let ticker = cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(std::time::Duration::from_secs(60)).await;
            if this.update(cx, |_, cx| cx.notify()).is_err() {
                break;
            }
        });
        Self {
            state,
            anchor: Local::now().date_naive(),
            day_count: DayCount::Workdays,
            nav: DateNav::default(),
            entries: Vec::new(),
            requested: None,
            load_task: None,
            selected: None,
            hovered_entry: String::new(),
            hover: None,
            column_bounds: Rc::default(),
            scroll: ScrollHandle::new(),
            initial_scroll_done: false,
            _ticker: ticker,
        }
    }

    pub fn nav_open(&self) -> bool {
        self.nav.open
    }

    pub fn zoom(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.state.update(cx, |s, _| {
            s.update_persist(|p| p.zoom_index = (p.zoom_index as i32 + delta).clamp(0, ZOOM_LEVELS.len() as i32 - 1) as usize)
        });
        cx.notify();
    }

    fn hour_px(&self, cx: &gpui::App) -> f32 {
        ZOOM_LEVELS[self.state.read(cx).persist.zoom_index.min(ZOOM_LEVELS.len() - 1)]
    }

    fn effective(&self, window: &Window) -> DayCount {
        effective_day_count(self.day_count, f32::from(window.viewport_size().width))
    }

    pub fn days(&self, window: &Window, cx: &gpui::App) -> Vec<NaiveDate> {
        let work_days = self.state.read(cx).data.as_ref().map(|d| d.settings.work_days.clone()).unwrap_or_default();
        visible_days(self.anchor, self.effective(window), &work_days)
    }

    /// Re-fetches `/entries` for the visible range.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let Some((from, to)) = self.requested else { return };
        let api = self.state.read(cx).api.clone();
        let fut = api.entries(at_minutes(from, 0).to_utc(), at_minutes(to, 0).to_utc());
        self.load_task = Some(cx.spawn(async move |this, cx| {
            if let Ok(entries) = fut.await {
                let _ = this.update(cx, |this, cx| {
                    if this.requested == Some((from, to)) {
                        this.entries = entries;
                        cx.notify();
                    }
                });
            }
        }));
    }

    fn ensure_loaded(&mut self, days: &[NaiveDate], cx: &mut Context<Self>) {
        let (Some(first), Some(last)) = (days.first(), days.last()) else { return };
        let key = (*first, *last + Duration::days(1));
        if self.requested != Some(key) {
            self.requested = Some(key);
            self.reload(cx);
        }
    }

    fn initial_scroll(&mut self, hour_px: f32, cx: &gpui::App) {
        if self.initial_scroll_done {
            return;
        }
        let Some(data) = self.state.read(cx).data.as_ref() else { return };
        let now = Local::now();
        let now_min = (now.hour() * 60 + now.minute()) as i64;
        let target = (data.settings.work_start.min(now_min) - 60).max(0);
        self.scroll.set_offset(point(px(0.), -px(BUFFER_PX + minutes_to_y(target, hour_px))));
        self.initial_scroll_done = true;
    }

    fn render_toolbar(&mut self, eff: DayCount, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let kind = match eff {
            DayCount::One => NavKind::Day,
            DayCount::Three => NavKind::Days(3),
            _ => NavKind::Week,
        };
        let width = f32::from(window.viewport_size().width);
        let options = [(DayCount::One, 0.), (DayCount::Three, 640.), (DayCount::Workdays, 768.), (DayCount::Seven, 768.)];
        controls::toolbar().child(date_nav::render(self.nav, self.anchor, kind, cx)).child(
            controls::group().children(options.into_iter().filter(|(_, min)| width >= *min).map(|(dc, _)| {
                controls::button(dc.label(), eff == dc, false).child(dc.label()).on_click(cx.listener(move |this, _, _, cx| {
                    this.day_count = dc;
                    cx.notify();
                }))
            })),
        )
    }

    fn render_day_header(&self, days: &[NaiveDate], zoom_index: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let today = Local::now().date_naive();
        let zoom_btn = |id: &'static str, label: &'static str, disabled: bool, delta: i32| {
            div()
                .id(id)
                .size(px(20.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .text_color(n500())
                .map(|d| if disabled { d.opacity(0.3) } else { d.cursor_pointer().hover(|s| s.bg(n100())) })
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if !disabled {
                        this.zoom(delta, cx)
                    }
                }))
        };
        div()
            .flex()
            .flex_shrink_0()
            .border_b_1()
            .border_color(n200())
            .child(
                div()
                    .w(px(GUTTER_PX))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(px(2.))
                    .child(zoom_btn("zoom-out", "−", zoom_index == 0, -1))
                    .child(zoom_btn("zoom-in", "+", zoom_index == ZOOM_LEVELS.len() - 1, 1)),
            )
            .children(days.iter().map(|&d| {
                let total = day_total_minutes(&self.entries, d);
                let holiday = holiday_name(d);
                div()
                    .flex_1()
                    .min_w_0()
                    .border_l_1()
                    .border_color(n200())
                    .px_2()
                    .py_2()
                    .text_xs()
                    .when(d == today, |s| s.bg(amber50()))
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap_2()
                            .child(
                                div()
                                    .size(px(40.))
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_full()
                                    .bg(with_alpha(n200(), 0.5))
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .child(d.day().to_string()),
                            )
                            .child(
                                div().flex().min_w_0().flex_1().flex_col().gap(px(2.)).child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .h(px(20.))
                                        .child(div().text_color(n500()).font_weight(FontWeight::SEMIBOLD).child(weekday_ja(d)))
                                        .when_some(holiday, |s, name| {
                                            s.child(div().rounded_sm().bg(gpui::rgb(0xf87171)).px_1().py(px(2.)).text_size(px(10.)).line_height(px(12.)).text_color(white()).truncate().child(name))
                                        }),
                                ),
                            )
                            .when(total > 0, |s| {
                                s.child(div().self_end().text_xs().text_color(n600()).font_weight(FontWeight::SEMIBOLD).child(format_duration_calendar(total)))
                            }),
                    )
            }))
    }

    fn render_gutter(&self, hour_px: f32) -> impl IntoElement {
        let height = 24. * hour_px + 1. + BUFFER_PX * 2.;
        let label = |top: f32, text: String, half: bool| {
            div()
                .absolute()
                .right_2()
                .top(px(top - if half { 6. } else { 7. }))
                .text_size(px(if half { 9. } else { 10. }))
                .line_height(px(if half { 12. } else { 14. }))
                .text_color(if half { n400() } else { n600() })
                .child(text)
        };
        div()
            .relative()
            .flex_shrink_0()
            .w(px(GUTTER_PX))
            .h(px(height))
            .children((0..=24).map(|h| label(BUFFER_PX + h as f32 * hour_px, format!("{h:02}:00"), false)))
            .when(hour_px >= 96., |d| d.children((0..24).map(|h| label(BUFFER_PX + h as f32 * hour_px + hour_px / 2., format!("{h:02}:30"), true))))
    }

    fn render_column(&self, day_index: usize, day: NaiveDate, hour_px: f32, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let settings = state.data.as_ref().map(|d| d.settings.clone());
        let is_work_day = settings.as_ref().is_some_and(|s| s.work_days.contains(&day.weekday().num_days_from_sunday())) && holiday_name(day).is_none();
        let body_h = 24. * hour_px + 1.;
        let bounds_store = self.column_bounds.clone();
        let day_entries: Vec<&TimeEntry> = entries_for_day(&self.entries, day);
        let spans: Vec<(i64, i64)> = day_entries.iter().map(|e| {
            let (s, e2) = entry_span_minutes(e, day);
            (s, e2.min(DAY_MINUTES))
        }).collect();
        let mut laid = layout_blocks(&spans, |s| *s);
        // GPUI paints in child order: draw the selected block last so it sits on top (z-index: 2).
        laid.sort_by_key(|l| self.selected.as_deref() == Some(day_entries[l.index].id.as_str()));
        let now = Local::now();
        let hover = self.hover.filter(|(i, _)| *i == day_index).map(|(_, m)| m);

        let hatch = || canvas(|_, _, _| (), |b, _, w, cx| grid::paint_hatch(b, w, cx)).w_full().h(px(BUFFER_PX));
        let body = div()
            .id(("day-col", day_index))
            .relative()
            .h(px(body_h))
            .cursor(gpui::CursorStyle::Crosshair)
            .bg(with_alpha(n200(), 0.2))
            .child(
                canvas(move |b, _, _| {
                    let mut v = bounds_store.borrow_mut();
                    if v.len() <= day_index {
                        v.resize(day_index + 1, Bounds::default());
                    }
                    v[day_index] = b;
                }, |_, _, _, _| ())
                .absolute()
                .size_full(),
            )
            .when_some(settings.filter(|_| is_work_day), |d, s| {
                d.child(div().absolute().left_0().right_0().top(px(minutes_to_y(s.work_start, hour_px))).h(px(minutes_to_y(s.work_end - s.work_start, hour_px).max(0.))).bg(white()))
            })
            .child(canvas(|_, _, _| (), move |b, _, w, cx| grid::paint_grid_lines(b, hour_px, w, cx)).absolute().size_full())
            .children(laid.iter().map(|l| {
                let e = day_entries[l.index];
                let g = BlockGeometry { start_min: spans[l.index].0, end_min: spans[l.index].1, lane: l.lane, lane_count: l.lane_count };
                let selected = self.selected.as_deref() == Some(e.id.as_str());
                let id = e.id.clone();
                let id2 = e.id.clone();
                lane_slot(&g, hour_px).child(
                    render_block(e, selected, false, &self.hovered_entry)
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            if *hovered {
                                this.hovered_entry = id.clone();
                            } else if this.hovered_entry == id {
                                this.hovered_entry.clear();
                            }
                            cx.notify();
                        }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selected = Some(id2.clone());
                            cx.notify();
                        })),
                )
            }))
            .when(day == now.date_naive(), |d| {
                let m = (now.hour() * 60 + now.minute()) as i64;
                d.child(div().absolute().left_0().right_0().top(px(minutes_to_y(m, hour_px))).h(px(1.)).bg(red500()))
            })
            .when_some(hover, |d, m| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px(minutes_to_y(m, hour_px) - 1.))
                        .h(px(1.))
                        .bg(n400())
                        .child(
                            div()
                                .absolute()
                                .right_full()
                                .top(px(-7.))
                                .rounded_full()
                                .bg(n700())
                                .px_1()
                                .text_size(px(9.))
                                .line_height(px(13.))
                                .text_color(white())
                                .child(format!("{:02}:{:02}", m / 60, m % 60)),
                        ),
                )
            })
            .on_mouse_move(cx.listener(move |this, ev: &gpui::MouseMoveEvent, _, cx| {
                let Some(b) = this.column_bounds.borrow().get(day_index).copied() else { return };
                let m = y_to_minutes(f32::from(ev.position.y - b.origin.y), this.hour_px(cx));
                if this.hover != Some((day_index, m)) {
                    this.hover = Some((day_index, m));
                    cx.notify();
                }
            }))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if !*hovered && this.hover.is_some_and(|(i, _)| i == day_index) {
                    this.hover = None;
                    cx.notify();
                }
            }))
            .on_mouse_down(gpui::MouseButton::Left, cx.listener(|this, _, _, cx| {
                this.selected = None;
                cx.notify();
            }));

        div()
            .relative()
            .flex_1()
            .min_w_0()
            .border_l_1()
            .border_color(n200())
            .child(hatch())
            .child(body)
            .child(hatch())
            .into_any_element()
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
        let days = self.days(window, cx);
        self.ensure_loaded(&days, cx);
        let hour_px = self.hour_px(cx);
        self.initial_scroll(hour_px, cx);
        let zoom_index = self.state.read(cx).persist.zoom_index;
        self.column_bounds.borrow_mut().truncate(days.len());
        let columns: Vec<AnyElement> = days.iter().enumerate().map(|(i, &d)| self.render_column(i, d, hour_px, cx)).collect();
        div()
            .size_full()
            .flex()
            .flex_col()
            .child(self.render_toolbar(eff, window, cx))
            .child(self.render_day_header(&days, zoom_index, cx))
            .child(
                div()
                    .id("calendar-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .child(div().flex().items_start().child(self.render_gutter(hour_px)).child(div().flex_1().flex().items_start().children(columns))),
            )
    }
}
