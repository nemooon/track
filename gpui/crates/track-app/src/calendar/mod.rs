mod day_menu;
mod edit_dialog;
mod entry_block;
mod grid;
mod picker;

use std::collections::HashMap;
use track_core::model::{EntryPatch, NewEntry, TimeEntry};
use crate::app::AppState;
use crate::core::interaction::{self, DragState, Edge, EntryCache};
use crate::core::geometry::{BUFFER_PX, GUTTER_PX, ZOOM_LEVELS, at_minutes, minutes_to_y, y_to_minutes};
use crate::core::holidays::holiday_name;
use crate::core::layout::{day_total_minutes, entries_for_day, entry_span_minutes, layout_blocks};
use crate::core::time::{DAY_MINUTES, DayCount, effective_day_count, format_duration, step_days, visible_days, weekday_ja};
use crate::ui::{controls, date_nav::{self, DateNav, HasDateNav, NavEvent, NavKind}, theme::*};
use chrono::{Datelike, Duration, Local, NaiveDate, Timelike};
use edit_dialog::{EditEvent, EditForm};
use entry_block::{BlockGeometry, BlockLook, duration_beside_title, lane_slot, render_block, sticky_title_offset};
use picker::{PICKER_WIDTH, PickerEvent, ProjectPicker};
use gpui::{
    AnyElement, App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable, FontWeight, InteractiveElement, IntoElement, KeyDownEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement, Pixels, Point, Render, ScrollHandle,
    StatefulInteractiveElement, Styled, Subscription, Task, Window, anchored, canvas, deferred, div, point, prelude::FluentBuilder, px,
};
use gpui_component::{Icon, IconName, WindowExt, button::Button, notification::Notification};
use std::cell::RefCell;
use std::rc::Rc;

pub struct CalendarView {
    state: Entity<AppState>,
    pub anchor: NaiveDate,
    pub day_count: DayCount,
    nav: DateNav,
    pub cache: EntryCache,
    /// Stored leave rows of the shown days (holidays' automatic leave is worked out when drawn).
    pub leaves: HashMap<NaiveDate, track_core::model::LeaveKind>,
    /// The day whose 休暇・直行直帰 menu is open.
    day_menu: Option<NaiveDate>,
    pub load_error: Option<String>,
    pub drag: DragState,
    pub editing: Option<String>,
    pub picker: Option<(Entity<ProjectPicker>, Subscription)>,
    edit_sub: Option<Subscription>,
    focus: FocusHandle,
    rendered_days: Vec<NaiveDate>,
    window: Option<gpui::AnyWindowHandle>,
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
            cache: EntryCache::default(),
            leaves: HashMap::new(),
            day_menu: None,
            load_error: None,
            drag: DragState::Idle,
            editing: None,
            picker: None,
            edit_sub: None,
            focus: cx.focus_handle(),
            rendered_days: Vec::new(),
            window: None,
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

    #[cfg(test)]
    pub fn nav_open(&self) -> bool {
        self.nav.open
    }

    pub fn zoom(&mut self, delta: i32, cx: &mut Context<Self>) {
        self.state.update(cx, |s, _| {
            s.update_persist(|p| p.zoom_index = (p.zoom_index as i32 + delta).clamp(0, ZOOM_LEVELS.len() as i32 - 1) as usize)
        });
        cx.notify();
    }

    /// A duration as the reports show it (the user's 「1.5h」/「1h 30m」 choice).
    pub fn duration(&self, minutes: i64, cx: &gpui::App) -> String {
        format_duration(minutes, self.state.read(cx).persist.duration_style)
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
        let leaves = api.leaves(from, to);
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let result = fut.await;
            let leaves = leaves.await;
            let _ = this.update(cx, |this, cx| {
                if this.requested != Some((from, to)) {
                    return;
                }
                match result {
                    Ok(entries) => {
                        this.cache = EntryCache::new(entries);
                        this.load_error = None;
                    }
                    Err(e) => this.load_error = Some(e.message),
                }
                match leaves {
                    Ok(rows) => this.leaves = rows.into_iter().map(|l| (l.date, l.kind)).collect(),
                    Err(e) => this.load_error = Some(e.message),
                }
                cx.notify();
            });
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

    /// Tests: start scrolled to an hour before 09:00, as during the day, whatever the clock says
    /// (at night the real first scroll goes to the current time).
    #[cfg(test)]
    pub fn pin_daytime_scroll(&mut self, cx: &gpui::App) {
        self.scroll.set_offset(point(px(0.), -px(BUFFER_PX + minutes_to_y(8 * 60, self.hour_px(cx)))));
        self.initial_scroll_done = true;
    }

    /// Minutes recorded on the shown days (the same per-day totals as the day headers).
    pub fn visible_total_minutes(&self, days: &[NaiveDate]) -> i64 {
        days.iter().map(|d| day_total_minutes(self.cache.entries(), *d)).sum()
    }

    /// 「<タイトル> HH:MM–HH:MM（H:MM）」 for the selected entry.
    pub fn selection_label(&self) -> Option<String> {
        let e = self.selected.as_ref().and_then(|id| self.cache.get(id))?;
        let (s, t) = (e.start.with_timezone(&Local), e.end.with_timezone(&Local));
        let end = if t.date_naive() > s.date_naive() && t.hour() == 0 && t.minute() == 0 { "24:00".to_string() } else { t.format("%H:%M").to_string() };
        let net = ((e.end - e.start).num_minutes() - e.break_minutes).max(0);
        let title = e.title.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| "（タイトルなし）".into());
        Some(format!("{title} {}–{end}（{}:{:02}）", s.format("%H:%M"), net / 60, net % 60))
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

    fn render_toolbar(&mut self, eff: DayCount, zoom_index: usize, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let kind = match eff {
            DayCount::One => NavKind::Day,
            DayCount::Three => NavKind::Days(3),
            _ => NavKind::Week,
        };
        let width = f32::from(window.viewport_size().width);
        let options = [(DayCount::One, 0.), (DayCount::Three, 640.), (DayCount::Workdays, 768.), (DayCount::Seven, 768.)];
        // Zoom sits beside the day count, like the other view controls.
        let zoom_btn = |id: &'static str, icon: IconName, tip: &'static str, disabled: bool, delta: i32, cx: &mut Context<Self>| {
            controls::disabled(
                controls::button(&p, id, false, true)
                    .debug_selector(move || id.into())
                    .child(Icon::new(icon).size_4())
                    .tooltip(move |window, cx| gpui_component::tooltip::Tooltip::new(tip).build(window, cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !disabled {
                            this.zoom(delta, cx)
                        }
                    })),
                disabled,
            )
        };
        let zoom_out = zoom_btn("zoom-out", IconName::Minus, "縮小（⌘-）", zoom_index == 0, -1, cx);
        let zoom_in = zoom_btn("zoom-in", IconName::Plus, "拡大（⌘+）", zoom_index == ZOOM_LEVELS.len() - 1, 1, cx);
        controls::toolbar(&p)
            .child(date_nav::render(self.nav, self.anchor, kind, cx))
            .child(controls::group(&p).children(options.into_iter().filter(|(_, min)| width >= *min).map(|(dc, _)| {
                controls::button(&p, dc.label(), eff == dc, false).child(dc.label()).on_click(cx.listener(move |this, _, _, cx| {
                    this.day_count = dc;
                    cx.notify();
                }))
            })))
            .child(
                controls::group(&p)
                    .child(zoom_out)
                    .child(zoom_in),
            )
    }

    /// One row per day: the date (a pill for today), weekday, holiday and the day's total.
    fn render_day_header(&self, days: &[NaiveDate], cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let today = Local::now().date_naive();
        div()
            .debug_selector(|| "day-header".into())
            .flex()
            .flex_shrink_0()
            .border_b_1()
            .border_color(p.border)
            .child(div().w(px(GUTTER_PX)).flex_shrink_0())
            .children(days.iter().map(|&d| {
                let total = day_total_minutes(self.cache.entries(), d);
                let holiday = holiday_name(d);
                div()
                    .flex_1()
                    .min_w_0()
                    .h(px(36.))
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_l_1()
                    .border_color(p.border)
                    .px_2()
                    .text_xs()
                    .child(
                        div()
                            .h(px(24.))
                            .min_w(px(24.))
                            .px_1()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .when(d == today, |c| c.bg(p.accent).text_color(p.accent_text))
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .child(d.day().to_string()),
                    )
                    .child(div().flex_shrink_0().text_color(p.text_muted).font_weight(FontWeight::SEMIBOLD).child(weekday_ja(d)))
                    .when_some(holiday, |s, name| {
                        s.child(div().min_w_0().rounded(RADIUS_SM).bg(p.holiday).px_1().py(px(2.)).text_size(px(10.)).line_height(px(12.)).text_color(p.on_color).truncate().child(name))
                    })
                    .children(self.day_mark(d, cx))
                    .child(div().flex_1())
                    .child(self.render_day_menu(d, cx))
                    .when(total > 0, |s| s.child(div().flex_shrink_0().text_color(p.text_muted).font_weight(FontWeight::SEMIBOLD).child(self.duration(total, cx))))
            }))
    }

    fn render_gutter(&self, p: Palette, hour_px: f32) -> impl IntoElement {
        let height = 24. * hour_px + 1. + BUFFER_PX * 2.;
        let label = |top: f32, text: String, half: bool| {
            div()
                .absolute()
                .right_2()
                .top(px(top - if half { 6. } else { 7. }))
                .text_size(px(if half { 9. } else { 10. }))
                .line_height(px(if half { 12. } else { 14. }))
                .font_family(MONO)
                .text_color(if half { p.text_faint } else { p.text_muted })
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

    fn render_column(&self, day_index: usize, day: NaiveDate, rendered: &[TimeEntry], ghost_id: Option<&str>, hour_px: f32, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let state = self.state.read(cx);
        let settings = state.data.as_ref().map(|d| d.settings.clone());
        let is_work_day = settings.as_ref().is_some_and(|s| s.work_days.contains(&day.weekday().num_days_from_sunday())) && holiday_name(day).is_none();
        let body_h = 24. * hour_px + 1.;
        let bounds_store = self.column_bounds.clone();
        // A 直行直帰 day shows in the header, not in the time grid.
        let day_entries: Vec<&TimeEntry> = entries_for_day(rendered, day).into_iter().filter(|e| e.kind != track_core::model::EntryKind::Direct).collect();
        let spans: Vec<(i64, i64)> = day_entries.iter().map(|e| {
            let (s, e2) = entry_span_minutes(e, day);
            (s, e2.min(DAY_MINUTES))
        }).collect();
        let mut laid = layout_blocks(&spans, |s| *s);
        // GPUI paints in child order: draw the selected block last so it sits on top (z-index: 2).
        laid.sort_by_key(|l| self.selected.as_deref() == Some(day_entries[l.index].id.as_str()));
        let now = Local::now();
        let style = state.persist.duration_style;
        // The top of the scrolled view in this column's coordinates (the body starts below the buffer).
        let visible_top = -f32::from(self.scroll.offset().y) - BUFFER_PX;
        let col_width = self.column_bounds.borrow().get(day_index).map(|b| f32::from(b.size.width)).unwrap_or(200.);
        let hover = self.display_hover().filter(|(i, _)| *i == day_index).map(|(_, m)| m);

        let hatch = || canvas(|_, _, _| (), |b, _, w, cx| grid::paint_hatch(b, w, cx)).w_full().h(px(BUFFER_PX));
        let body = div()
            .id(("day-col", day_index))
            .relative()
            .h(px(body_h))
            .cursor(gpui::CursorStyle::Crosshair)
            .bg(with_alpha(p.border, 0.2))
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
                d.child(div().absolute().left_0().right_0().top(px(minutes_to_y(s.work_start, hour_px))).h(px(minutes_to_y(s.work_end - s.work_start, hour_px).max(0.))).bg(p.bg))
            })
            .child(canvas(|_, _, _| (), move |b, _, w, cx| grid::paint_grid_lines(b, hour_px, w, cx)).absolute().size_full())
            .children(laid.iter().map(|l| {
                let e = day_entries[l.index];
                let g = BlockGeometry { start_min: spans[l.index].0, end_min: spans[l.index].1, lane: l.lane, lane_count: l.lane_count, span: l.span };
                // The title stays in view while the block's top is scrolled out; a narrow block puts
                // its duration under the title.
                let block_top = minutes_to_y(g.start_min, hour_px) + 2.;
                let block_h = (minutes_to_y(g.end_min - g.start_min, hour_px) - 5.).max(14.);
                let width = col_width * g.span.max(1) as f32 / g.lane_count.max(1) as f32 - 6.;
                let look = BlockLook { style, title_top: sticky_title_offset(block_top, block_h, visible_top), beside: duration_beside_title(width) };
                let ghost = ghost_id == Some(e.id.as_str());
                let selected = !ghost && self.selected.as_deref() == Some(e.id.as_str());
                let (id_hover, id_down, id_click, id_top, id_bottom) = (e.id.clone(), e.id.clone(), e.id.clone(), e.id.clone(), e.id.clone());
                let handle = |edge: Edge, id: String| {
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .h(px(6.))
                        .map(|d| if edge == Edge::Top { d.top_0() } else { d.bottom_0() })
                        .cursor(gpui::CursorStyle::ResizeUpDown)
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, w, cx| {
                            cx.stop_propagation();
                            this.begin_resize(&id, edge, ev.position, w, cx);
                        }))
                };
                lane_slot(&g, hour_px).child(
                    render_block(p, e, selected, ghost, &self.hovered_entry, look)
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            if *hovered {
                                this.hovered_entry = id_hover.clone();
                            } else if this.hovered_entry == id_hover {
                                this.hovered_entry.clear();
                            }
                            cx.notify();
                        }))
                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, w, cx| {
                            cx.stop_propagation();
                            this.begin_move(&id_down, ev.position, w, cx);
                        }))
                        .on_click(cx.listener(move |this, ev: &gpui::ClickEvent, w, cx| {
                            this.selected = Some(id_click.clone());
                            if ev.click_count() >= 2 {
                                this.open_edit(&id_click, w, cx);
                            }
                            cx.notify();
                        }))
                        .when(!ghost, |d| d.child(handle(Edge::Top, id_top)).child(handle(Edge::Bottom, id_bottom))),
                )
            }))
            .map(|d| match &self.drag {
                DragState::Creating { day_index: di, anchor_min, current_min } if *di == day_index => {
                    let (a, b) = (*anchor_min.min(current_min), *anchor_min.max(current_min));
                    d.child(placeholder(a, b, hour_px, p.text_muted, with_alpha(p.text_faint, 0.3)))
                }
                DragState::Picking { day_index: di, start_min, end_min } if *di == day_index => {
                    d.child(placeholder(*start_min, *end_min, hour_px, p.text_faint, with_alpha(p.border, 0.5)))
                }
                _ => d,
            })
            .when(day == now.date_naive(), |d| {
                let m = (now.hour() * 60 + now.minute()) as i64;
                d.child(div().absolute().left_0().right_0().top(px(minutes_to_y(m, hour_px))).h(px(1.)).bg(p.now_line))
            })
            .when_some(hover, |d, m| {
                d.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px(minutes_to_y(m, hour_px) - 1.))
                        .h(px(1.))
                        .bg(p.text_faint)
                        .child(
                            div()
                                .debug_selector(|| "hover-time".into())
                                .absolute()
                                .right_full()
                                .top(px(-7.))
                                .whitespace_nowrap()
                                .rounded_full()
                                .bg(p.primary)
                                .px_1()
                                .text_size(px(9.))
                                .line_height(px(13.))
                                .text_color(p.on_color)
                                .child(format!("{:02}:{:02}", m / 60, m % 60)),
                        ),
                )
            })
            .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _, cx| {
                if this.drag != DragState::Idle {
                    return;
                }
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
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _, cx| {
                this.selected = None;
                if matches!(this.drag, DragState::Picking { .. }) {
                    this.close_picker(cx);
                    return;
                }
                if let Some(m) = this.pointer_minutes(day_index, ev.position, cx) {
                    this.drag = interaction::begin_create(day_index, m);
                }
                cx.notify();
            }));

        div()
            .relative()
            .flex_1()
            .min_w_0()
            .border_l_1()
            .border_color(p.border)
            .child(hatch())
            .child(body)
            .child(hatch())
            .into_any_element()
    }
}

fn placeholder(a: i64, b: i64, hour_px: f32, border: gpui::Hsla, bg: gpui::Hsla) -> impl IntoElement {
    div()
        .absolute()
        .left(px(3.))
        .right(px(3.))
        .top(px(minutes_to_y(a, hour_px) + 1.))
        .h(px((minutes_to_y(b - a, hour_px) - 3.).max(2.)))
        .rounded(RADIUS_SM)
        .border_1()
        .border_dashed()
        .border_color(border)
        .bg(bg)
}

impl CalendarView {
    fn pointer_minutes(&self, day_index: usize, pos: Point<Pixels>, cx: &App) -> Option<i64> {
        let b = self.column_bounds.borrow().get(day_index).copied()?;
        Some(y_to_minutes(f32::from(pos.y - b.origin.y), self.hour_px(cx)))
    }

    fn hit_day(&self, pos: Point<Pixels>) -> Option<usize> {
        self.column_bounds.borrow().iter().position(|b| pos.x >= b.origin.x && pos.x < b.origin.x + b.size.width)
    }

    /// Hover line: follows the cursor when idle, or the active edge while dragging.
    fn display_hover(&self) -> Option<(usize, i64)> {
        match &self.drag {
            DragState::Creating { day_index, current_min, .. } => Some((*day_index, *current_min)),
            DragState::Moving { day_index, start_min, duration, .. } => Some((*day_index, start_min + duration)),
            DragState::Resizing { day_index, edge, start_min, end_min, .. } => Some((*day_index, if *edge == Edge::Top { *start_min } else { *end_min })),
            DragState::Idle => self.hover,
            DragState::Picking { .. } => None,
        }
    }

    fn day_index_of(&self, e: &TimeEntry) -> Option<usize> {
        let d = e.start.with_timezone(&Local).date_naive();
        self.rendered_days.iter().position(|x| *x == d)
    }

    fn begin_move(&mut self, id: &str, pos: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        let Some(e) = self.cache.get(id).cloned() else { return };
        let Some(di) = self.day_index_of(&e) else { return };
        let day = self.rendered_days[di];
        let (s, en) = entry_span_minutes(&e, day);
        let Some(pm) = self.pointer_minutes(di, pos, cx) else { return };
        self.selected = Some(id.to_string());
        self.drag = DragState::Moving { id: id.into(), day_index: di, start_min: s, duration: en - s, offset_min: pm - s, orig_day_index: di, orig_start_min: s };
        cx.notify();
    }

    fn begin_resize(&mut self, id: &str, edge: Edge, _pos: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
        let Some(e) = self.cache.get(id).cloned() else { return };
        let Some(di) = self.day_index_of(&e) else { return };
        let (s, en) = entry_span_minutes(&e, self.rendered_days[di]);
        self.selected = Some(id.to_string());
        self.drag = DragState::Resizing { id: id.into(), day_index: di, edge, start_min: s, end_min: en, orig: (s, en) };
        cx.notify();
    }

    fn on_drag_move(&mut self, pos: Point<Pixels>, cx: &mut Context<Self>) {
        let next = match &self.drag {
            DragState::Creating { day_index, .. } => match self.pointer_minutes(*day_index, pos, cx) {
                Some(m) => interaction::update_create(&self.drag, m),
                None => return,
            },
            DragState::Moving { day_index, .. } => {
                let hit = self.hit_day(pos);
                match self.pointer_minutes(hit.unwrap_or(*day_index), pos, cx) {
                    Some(m) => interaction::update_move(&self.drag, hit, m),
                    None => return,
                }
            }
            DragState::Resizing { day_index, .. } => match self.pointer_minutes(*day_index, pos, cx) {
                Some(m) => interaction::update_resize(&self.drag, m),
                None => return,
            },
            _ => return,
        };
        if next != self.drag {
            self.drag = next;
            cx.notify();
        }
    }

    fn on_drag_end(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let drag = std::mem::take(&mut self.drag);
        match drag {
            DragState::Creating { .. } => {
                if let Some(p) = interaction::finish_create(&drag) {
                    self.drag = p;
                    self.open_picker(window, cx);
                }
            }
            DragState::Moving { id, day_index, start_min, duration, orig_day_index, orig_start_min, .. } => {
                if let (true, Some(&day)) = ((day_index, start_min) != (orig_day_index, orig_start_min), self.rendered_days.get(day_index)) {
                    self.commit_times(&id, at_minutes(day, start_min).to_utc(), at_minutes(day, start_min + duration).to_utc(), window, cx);
                }
            }
            DragState::Resizing { id, day_index, start_min, end_min, orig, .. } => {
                if let (true, Some(&day)) = ((start_min, end_min) != orig, self.rendered_days.get(day_index)) {
                    self.commit_times(&id, at_minutes(day, start_min).to_utc(), at_minutes(day, end_min).to_utc(), window, cx);
                }
            }
            other => self.drag = other,
        }
        cx.notify();
    }

    /// Entries as drawn: the one being moved/resized is replaced by its ghost.
    fn rendered_entries(&self) -> (Vec<TimeEntry>, Option<String>) {
        let mut list = self.cache.snapshot();
        let ghost = match &self.drag {
            DragState::Moving { id, day_index, start_min, duration, .. } => Some((id.clone(), *day_index, *start_min, start_min + duration)),
            DragState::Resizing { id, day_index, start_min, end_min, .. } => Some((id.clone(), *day_index, *start_min, *end_min)),
            _ => None,
        };
        let Some((id, di, s, e)) = ghost else { return (list, None) };
        if let (Some(entry), Some(day)) = (list.iter_mut().find(|x| x.id == id), self.rendered_days.get(di)) {
            entry.start = at_minutes(*day, s).to_utc();
            entry.end = at_minutes(*day, e).to_utc();
        }
        (list, Some(id))
    }

    fn notify_msg(&self, n: Notification, cx: &mut Context<Self>) {
        let n = n.autohide(true);
        if let Some(w) = self.window {
            let _ = w.update(cx, |_, window, cx| window.push_notification(n, cx));
        }
    }

    /// Optimistic PATCH of start/end (updateEntry in WeekCalendar.tsx).
    pub fn commit_times(&mut self, id: &str, start: chrono::DateTime<chrono::Utc>, end: chrono::DateTime<chrono::Utc>, window: &mut Window, cx: &mut Context<Self>) {
        let patch = EntryPatch { start: Some(start), end: Some(end), ..Default::default() };
        let snapshot = self.cache.snapshot();
        self.cache.apply(id, start, end);
        self.commit_patch(id, patch, snapshot, window, cx);
    }

    /// `snapshot` is the cache before the optimistic change; restored on failure.
    pub fn commit_patch(&mut self, id: &str, patch: EntryPatch, snapshot: Vec<TimeEntry>, window: &mut Window, cx: &mut Context<Self>) {
        self.window = Some(window.window_handle());
        let fut = self.state.read(cx).api.update_entry(id, patch);
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(updated) => {
                        this.cache.replace(updated);
                        this.notify_msg(Notification::success("保存しました"), cx);
                    }
                    Err(_) => {
                        this.cache.rollback(snapshot);
                        this.notify_msg(Notification::error("更新に失敗しました"), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn create(&mut self, body: NewEntry, window: &mut Window, cx: &mut Context<Self>) {
        self.window = Some(window.window_handle());
        let fut = self.state.read(cx).api.create_entry(body);
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(e) => this.cache.insert(e),
                    Err(_) => this.notify_msg(Notification::error("記録の作成に失敗しました"), cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Delete immediately; the toast offers 「元に戻す」 which re-creates the same entry.
    pub fn delete(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.window = Some(window.window_handle());
        let snapshot = self.cache.snapshot();
        let Some(removed) = self.cache.remove(id) else { return };
        if self.selected.as_deref() == Some(id) {
            self.selected = None;
        }
        let fut = self.state.read(cx).api.delete_entry(id);
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(_) => {
                        let weak = cx.weak_entity();
                        let n = Notification::success("削除しました").action(move |_, _, _| {
                            let weak = weak.clone();
                            let removed = removed.clone();
                            Button::new("undo-delete").label("元に戻す").on_click(move |_, window, cx| {
                                let body = NewEntry {
                                    project_id: removed.project_id.clone(),
                                    start: removed.start,
                                    end: removed.end,
                                    title: removed.title.clone(),
                                    note: removed.note.clone(),
                                    tag_ids: removed.tags.iter().map(|t| t.tag_id.clone()).collect(),
                                    external_event_id: removed.external_event_id.clone(),
                                    external_event_source: removed.external_event_source.clone(),
                                    break_minutes: removed.break_minutes,
                                };
                                let _ = weak.update(cx, |this, cx| this.create(body, window, cx));
                            })
                        });
                        this.notify_msg(n, cx);
                    }
                    Err(_) => {
                        this.cache.rollback(snapshot);
                        this.notify_msg(Notification::error("削除に失敗しました"), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.drag, DragState::Picking { .. }) && ev.keystroke.key == "escape" {
            self.close_picker(cx);
            return;
        }
        // Only when the calendar itself has focus (not a text field inside a popover/dialog).
        if !self.focus.is_focused(window) {
            return;
        }
        let Some(id) = self.selected.clone() else { return };
        let Some(e) = self.cache.get(&id).cloned() else { return };
        match ev.keystroke.key.as_str() {
            "backspace" | "delete" => self.delete(&id, window, cx),
            "escape" => {
                self.selected = None;
                cx.notify();
            }
            "up" | "down" => {
                let delta = if ev.keystroke.key == "up" { -15 } else { 15 };
                let day = e.start.with_timezone(&Local).date_naive();
                let (s, en) = entry_span_minutes(&e, day);
                if let Some((ns, ne)) = interaction::nudge(s, en, delta, ev.keystroke.modifiers.shift) {
                    self.commit_times(&id, at_minutes(day, ns).to_utc(), at_minutes(day, ne).to_utc(), window, cx);
                }
            }
            _ => {}
        }
    }
}

impl CalendarView {
    fn open_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        let Some(data) = state.data.clone() else { return };
        let api = state.api.clone();
        let picker = cx.new(|cx| ProjectPicker::new(data.projects.clone(), data.tags.clone(), api, window, cx));
        let sub = cx.subscribe_in(&picker, window, |this, _, ev: &PickerEvent, window, cx| {
            match ev.clone() {
                PickerEvent::Submit { project_id, title, note, tag_ids } => {
                    if let DragState::Picking { day_index, start_min, end_min } = this.drag
                        && let Some(day) = this.rendered_days.get(day_index).copied() {
                            let body = NewEntry {
                                project_id,
                                start: at_minutes(day, start_min).to_utc(),
                                end: at_minutes(day, end_min).to_utc(),
                                title: (!title.is_empty()).then_some(title),
                                note: (!note.is_empty()).then_some(note),
                                tag_ids,
                                ..Default::default()
                            };
                            this.create(body, window, cx);
                        }
                    this.close_picker(cx);
                    this.focus.focus(window, cx);
                }
                PickerEvent::Cancel => {
                    this.close_picker(cx);
                    this.focus.focus(window, cx);
                }
            }
        });
        self.picker = Some((picker, sub));
        cx.notify();
    }

    fn close_picker(&mut self, cx: &mut Context<Self>) {
        self.picker = None;
        if matches!(self.drag, DragState::Picking { .. }) {
            self.drag = DragState::Idle;
        }
        cx.notify();
    }

    /// Picker anchor: horizontally centred on the column, 4px below the selection.
    fn picker_position(&self, cx: &App) -> Option<Point<Pixels>> {
        let DragState::Picking { day_index, end_min, .. } = self.drag else { return None };
        let b = self.column_bounds.borrow().get(day_index).copied()?;
        Some(point(b.origin.x + b.size.width / 2. - px(PICKER_WIDTH / 2.), b.origin.y + px(minutes_to_y(end_min, self.hour_px(cx)) + 4.)))
    }

    pub fn open_edit(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.cache.get(id).cloned() else { return };
        let Some(data) = self.state.read(cx).data.clone() else { return };
        self.editing = Some(id.to_string());
        let form = cx.new(|cx| EditForm::new(entry, data.projects.clone(), data.tags.clone(), window, cx));
        self.edit_sub = Some(cx.subscribe_in(&form, window, |this, _, ev: &EditEvent, window, cx| {
            match ev.clone() {
                EditEvent::Save(id, patch) => {
                    let snapshot = this.cache.snapshot();
                    this.commit_patch(&id, patch, snapshot, window, cx);
                }
                EditEvent::Delete(id) => this.delete(&id, window, cx),
                EditEvent::Cancel => {}
            }
            this.editing = None;
            window.close_dialog(cx);
            this.focus.focus(window, cx);
        }));
        let weak = cx.weak_entity();
        let first = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            // open_dialog focuses the dialog shell; move focus to the title once per opening.
            if first.replace(false) {
                let title = form.read(cx).title_state();
                title.update(cx, |s, cx| s.focus(window, cx));
            }
            let weak = weak.clone();
            dialog
                .title("予定を編集")
                .w(px(512.))
                .child(form.clone())
                .on_close(move |_, _, cx| {
                    let _ = weak.update(cx, |this, _| this.editing = None);
                })
        });
        cx.notify();
    }
}

impl Focusable for CalendarView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl HasDateNav for CalendarView {
    fn date_nav(&mut self) -> &mut DateNav {
        &mut self.nav
    }

    fn on_nav(&mut self, ev: NavEvent, window: &mut Window, cx: &mut Context<Self>) {
        // A drag or pending pick refers to column indexes of the current range: cancel it.
        if self.drag != DragState::Idle {
            self.picker = None;
            self.drag = DragState::Idle;
        }
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
        let p = palette(cx);
        let eff = self.effective(window);
        let days = self.days(window, cx);
        self.ensure_loaded(&days, cx);
        let hour_px = self.hour_px(cx);
        self.initial_scroll(hour_px, cx);
        let zoom_index = self.state.read(cx).persist.zoom_index;
        self.column_bounds.borrow_mut().truncate(days.len());
        self.rendered_days = days.clone();
        self.window = Some(window.window_handle());
        let (rendered, ghost) = self.rendered_entries();
        let columns: Vec<AnyElement> = days.iter().enumerate().map(|(i, &d)| self.render_column(i, d, &rendered, ghost.as_deref(), hour_px, cx)).collect();
        let picker_overlay = self.picker.as_ref().zip(self.picker_position(cx)).map(|((p, _), pos)| {
            deferred(anchored().position(pos).snap_to_window_with_margin(px(8.)).child(p.clone())).with_priority(5)
        });
        div()
            .id("calendar-root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, w, cx| this.on_key(ev, w, cx)))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, w, cx| this.focus.focus(w, cx)))
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| this.on_drag_move(ev.position, cx)))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, w, cx| this.on_drag_end(w, cx)))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _: &MouseUpEvent, w, cx| this.on_drag_end(w, cx)))
            .size_full()
            .flex()
            .flex_col()
            .child(self.render_toolbar(eff, zoom_index, window, cx))
            .when_some(self.load_error.clone(), |d, msg| {
                d.child(controls::error_banner(&p, "calendar-retry", &msg).on_click(cx.listener(|this, _, _, cx| this.reload(cx))))
            })
            .child(self.render_day_header(&days, cx))
            .child(
                div()
                    .id("calendar-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    // Long blocks keep their titles in view: redraw as the view scrolls.
                    .on_scroll(cx.listener(|_, _, _, cx| cx.notify()))
                    .child(div().flex().items_start().child(self.render_gutter(p, hour_px)).child(div().flex_1().flex().items_start().children(columns))),
            )
            .children(picker_overlay)
    }
}
