//! DateRangeNavigator + DatePickerPopover.
use super::{controls, theme::*};
use crate::core::time::{month_range, range_label, week_start};
use chrono::{Datelike, Duration, Local, NaiveDate};
use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    Window, anchored, deferred, div, prelude::FluentBuilder, px,
};
use gpui_component::{Icon, IconName};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavKind {
    Day,
    Days(i64),
    Week,
    Month,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavEvent {
    Prev,
    Next,
    Today,
    Pick(NaiveDate),
}

#[derive(Debug, Clone, Copy)]
pub struct DateNav {
    pub open: bool,
    pub view_month: NaiveDate,
}

impl Default for DateNav {
    fn default() -> Self {
        Self { open: false, view_month: Local::now().date_naive().with_day(1).unwrap() }
    }
}

/// Inclusive (start, end) shown by the navigator.
pub fn bounds(anchor: NaiveDate, kind: NavKind) -> (NaiveDate, NaiveDate) {
    match kind {
        NavKind::Day => (anchor, anchor),
        NavKind::Days(n) => (anchor, anchor + Duration::days(n - 1)),
        NavKind::Week => {
            let s = week_start(anchor);
            (s, s + Duration::days(6))
        }
        NavKind::Month => {
            let (s, e) = month_range(anchor);
            (s, e - Duration::days(1))
        }
    }
}

pub trait HasDateNav: 'static + Sized {
    fn date_nav(&mut self) -> &mut DateNav;
    fn on_nav(&mut self, ev: NavEvent, window: &mut Window, cx: &mut Context<Self>);
}

pub fn render<V: HasDateNav>(nav: DateNav, anchor: NaiveDate, kind: NavKind, cx: &mut Context<V>) -> Div {
    let p = palette(cx);
    let (start, end) = bounds(anchor, kind);
    let today = Local::now().date_naive();
    let includes_today = start <= today && today <= end;
    let label = range_label(start, end + Duration::days(1), kind == NavKind::Month);
    controls::group(&p)
        .relative()
        .child(
            controls::button(&p, "nav-prev", false, true)
                .child(Icon::new(IconName::ChevronLeft).size_4())
                .tooltip(|window, cx| gpui_component::tooltip::Tooltip::new("前の期間（⌘[）").build(window, cx))
                .on_click(cx.listener(|this, _, w, cx| this.on_nav(NavEvent::Prev, w, cx))),
        )
        .child(
            controls::button(&p, "nav-label", true, false)
                .debug_selector(|| "nav-label".into())
                .w(px(176.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    let n = this.date_nav();
                    n.open = !n.open;
                    n.view_month = anchor.with_day(1).unwrap();
                    cx.notify();
                })),
        )
        .child(
            controls::button(&p, "nav-next", false, true)
                .child(Icon::new(IconName::ChevronRight).size_4())
                .tooltip(|window, cx| gpui_component::tooltip::Tooltip::new("次の期間（⌘]）").build(window, cx))
                .on_click(cx.listener(|this, _, w, cx| this.on_nav(NavEvent::Next, w, cx))),
        )
        .child(
            controls::button(&p, "nav-today", includes_today, false)
                .child("今日")
                .on_click(cx.listener(|this, _, w, cx| this.on_nav(NavEvent::Today, w, cx))),
        )
        .when(nav.open, |d| {
            d.child(below(picker(nav, anchor, kind, cx)))
        })
}

/// Overlay anchored to the bottom-left of the parent (which must be `.relative()`).
pub fn below(content: impl IntoElement) -> Div {
    div().absolute().top_full().left_0().child(deferred(anchored().snap_to_window_with_margin(px(8.)).child(content)).with_priority(10))
}

fn close<V: HasDateNav>(this: &mut V, cx: &mut Context<V>) {
    this.date_nav().open = false;
    cx.notify();
}

fn add_months(d: NaiveDate, months: i32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 + months;
    NaiveDate::from_ymd_opt(total.div_euclid(12), total.rem_euclid(12) as u32 + 1, 1).unwrap()
}

fn picker<V: HasDateNav>(nav: DateNav, value: NaiveDate, kind: NavKind, cx: &mut Context<V>) -> Div {
    let p = palette(cx);
    let today = Local::now().date_naive();
    let vm = nav.view_month;
    let shell = div()
        .debug_selector(|| "date-picker".into())
        .mt_1()
        .w(px(268.))
        .rounded_lg()
        .border_1()
        .border_color(p.border)
        .bg(p.bg)
        .p_3()
        .shadow_lg()
        .occlude()
        .on_mouse_down_out(cx.listener(|this, _, _, cx| close(this, cx)));
    let step = if kind == NavKind::Month { 12 } else { 1 };
    let header = div()
        .mb_2()
        .flex()
        .items_center()
        .justify_between()
        .child(controls::icon_button(&p, "pick-prev", Icon::new(IconName::ChevronLeft), "前の月").on_click(cx.listener(move |this, _, _, cx| {
            let n = this.date_nav();
            n.view_month = add_months(n.view_month, -step);
            cx.notify();
        })))
        .child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(if kind == NavKind::Month { format!("{}年", vm.year()) } else { format!("{}年 {}月", vm.year(), vm.month()) }),
        )
        .child(controls::icon_button(&p, "pick-next", Icon::new(IconName::ChevronRight), "次の月").on_click(cx.listener(move |this, _, _, cx| {
            let n = this.date_nav();
            n.view_month = add_months(n.view_month, step);
            cx.notify();
        })));

    let pick = move |d: NaiveDate| {
        move |this: &mut V, _: &gpui::ClickEvent, w: &mut Window, cx: &mut Context<V>| {
            this.date_nav().open = false;
            this.on_nav(NavEvent::Pick(d), w, cx);
        }
    };

    if kind == NavKind::Month {
        let months = (0..12u32).map(|m| {
            let d = NaiveDate::from_ymd_opt(vm.year(), m + 1, 1).unwrap();
            let selected = vm.year() == value.year() && m == value.month0();
            let this_month = vm.year() == today.year() && m == today.month0();
            div()
                .id(("month", m as usize))
                .w(px(76.))
                .py_2()
                .rounded_md()
                .text_sm()
                .flex()
                .justify_center()
                .cursor_pointer()
                .map(|s| {
                    if selected {
                        s.bg(p.primary).text_color(p.on_color).font_weight(gpui::FontWeight::SEMIBOLD)
                    } else if this_month {
                        s.text_color(p.accent).font_weight(gpui::FontWeight::BOLD).hover(move |h| h.bg(p.hover))
                    } else {
                        s.text_color(p.text).hover(|h| h.bg(p.hover))
                    }
                })
                .child(format!("{}月", m + 1))
                .on_click(cx.listener(pick(d)))
        });
        return shell.child(header).child(div().flex().flex_wrap().gap_1().children(months));
    }

    let grid_start = week_start(vm);
    let month_end = add_months(vm, 1) - Duration::days(1);
    let grid_end = week_start(month_end) + Duration::days(6);
    let weeks = ((grid_end - grid_start).num_days() + 1) / 7;
    let weekday_row = div().mb_1().flex().children(["日", "月", "火", "水", "木", "金", "土"].iter().enumerate().map(|(i, w)| {
        div()
            .flex_1()
            .flex()
            .justify_center()
            .py_1()
            .text_size(px(10.))
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(if i == 0 { p.sunday } else if i == 6 { p.saturday } else { p.text_muted })
            .child(*w)
    }));
    let day_color = move |d: NaiveDate, dow: usize, selected: bool| {
        if selected {
            p.on_color
        } else if d.month() != vm.month() {
            p.text_faint
        } else if d == today {
            p.accent
        } else if dow == 0 {
            p.sunday
        } else if dow == 6 {
            p.saturday
        } else {
            p.text
        }
    };
    let selected_week = week_start(value);
    let rows: Vec<AnyElement> = (0..weeks)
        .map(|w| {
            let ws = grid_start + Duration::days(w * 7);
            if kind == NavKind::Week {
                let selected = ws == selected_week;
                div()
                    .id(("week", w as usize))
                    .flex()
                    .rounded_md()
                    .p(px(2.))
                    .cursor_pointer()
                    .map(|s| if selected { s.bg(p.primary).font_weight(gpui::FontWeight::SEMIBOLD) } else { s.hover(|h| h.bg(p.hover)) })
                    .children((0..7).map(|i| {
                        let d = ws + Duration::days(i);
                        div()
                            .flex_1()
                            .flex()
                            .justify_center()
                            .py_1()
                            .text_xs()
                            .text_color(day_color(d, i as usize, selected))
                            .when(!selected && d == today, |s| s.font_weight(gpui::FontWeight::BOLD))
                            .child(d.day().to_string())
                    }))
                    .debug_selector(move || format!("pick-week-{ws}"))
                    .on_click(cx.listener(pick(ws)))
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .gap(px(2.))
                    .children((0..7).map(|i| {
                        let d = ws + Duration::days(i);
                        let selected = d == value;
                        div()
                            .id(("day", (w * 7 + i) as usize))
                            .flex_1()
                            .flex()
                            .justify_center()
                            .py_1()
                            .rounded_md()
                            .text_xs()
                            .cursor_pointer()
                            .text_color(day_color(d, i as usize, selected))
                            .map(|s| if selected { s.bg(p.primary).font_weight(gpui::FontWeight::SEMIBOLD) } else { s.hover(|h| h.bg(p.hover)) })
                            .when(!selected && d == today, |s| s.font_weight(gpui::FontWeight::BOLD))
                            .child(d.day().to_string())
                            .debug_selector(move || format!("pick-{d}"))
                            .on_click(cx.listener(pick(d)))
                    }))
                    .into_any_element()
            }
        })
        .collect();
    shell.child(header).child(weekday_row).child(div().flex().flex_col().gap(px(2.)).children(rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_per_kind() {
        let d = |m, day| NaiveDate::from_ymd_opt(2026, m, day).unwrap();
        assert_eq!(bounds(d(11, 12), NavKind::Week), (d(11, 8), d(11, 14)));
        assert_eq!(bounds(d(11, 12), NavKind::Days(3)), (d(11, 12), d(11, 14)));
        assert_eq!(bounds(d(11, 12), NavKind::Month), (d(11, 1), d(11, 30)));
        assert_eq!(add_months(d(1, 15), -1), NaiveDate::from_ymd_opt(2025, 12, 1).unwrap());
    }
}
