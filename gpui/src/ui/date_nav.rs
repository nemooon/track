//! DateRangeNavigator + DatePickerPopover.
use super::{controls, theme::*};
use crate::core::time::{month_range, range_label, week_start};
use chrono::{Datelike, Duration, Local, NaiveDate};
use gpui::{
    AnyElement, Context, Div, InteractiveElement, IntoElement, ParentElement, Stateful, StatefulInteractiveElement, Styled,
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
    let (start, end) = bounds(anchor, kind);
    let today = Local::now().date_naive();
    let includes_today = start <= today && today <= end;
    let label = range_label(start, end + Duration::days(1), kind == NavKind::Month);
    controls::group()
        .relative()
        .child(
            controls::button("nav-prev", false, true)
                .child(Icon::new(IconName::ChevronLeft).size_4())
                .on_click(cx.listener(|this, _, w, cx| this.on_nav(NavEvent::Prev, w, cx))),
        )
        .child(
            controls::button("nav-label", true, false)
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
            controls::button("nav-next", false, true)
                .child(Icon::new(IconName::ChevronRight).size_4())
                .on_click(cx.listener(|this, _, w, cx| this.on_nav(NavEvent::Next, w, cx))),
        )
        .child(
            controls::button("nav-today", includes_today, false)
                .child("今日")
                .on_click(cx.listener(|this, _, w, cx| this.on_nav(NavEvent::Today, w, cx))),
        )
        .when(nav.open, |d| {
            d.child(deferred(anchored().snap_to_window_with_margin(px(8.)).child(picker(nav, anchor, kind, cx))).with_priority(10))
        })
}

fn close<V: HasDateNav>(this: &mut V, cx: &mut Context<V>) {
    this.date_nav().open = false;
    cx.notify();
}

fn nav_icon_button(id: &'static str, icon: IconName) -> Stateful<Div> {
    div().id(id).rounded_sm().p_1().text_color(n600()).cursor_pointer().hover(|s| s.bg(n100())).child(Icon::new(icon).size_4())
}

fn add_months(d: NaiveDate, months: i32) -> NaiveDate {
    let total = d.year() * 12 + d.month0() as i32 + months;
    NaiveDate::from_ymd_opt(total.div_euclid(12), total.rem_euclid(12) as u32 + 1, 1).unwrap()
}

fn picker<V: HasDateNav>(nav: DateNav, value: NaiveDate, kind: NavKind, cx: &mut Context<V>) -> Div {
    let today = Local::now().date_naive();
    let vm = nav.view_month;
    let shell = div()
        .mt_1()
        .w(px(268.))
        .rounded_lg()
        .border_1()
        .border_color(n200())
        .bg(white())
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
        .child(nav_icon_button("pick-prev", IconName::ChevronLeft).on_click(cx.listener(move |this, _, _, cx| {
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
        .child(nav_icon_button("pick-next", IconName::ChevronRight).on_click(cx.listener(move |this, _, _, cx| {
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
                        s.bg(n900()).text_color(white()).font_weight(gpui::FontWeight::SEMIBOLD)
                    } else if this_month {
                        s.text_color(amber600()).font_weight(gpui::FontWeight::BOLD).hover(|h| h.bg(amber50()))
                    } else {
                        s.text_color(n700()).hover(|h| h.bg(n100()))
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
            .text_color(if i == 0 { red500() } else if i == 6 { blue500() } else { n500() })
            .child(*w)
    }));
    let day_color = move |d: NaiveDate, dow: usize, selected: bool| {
        if selected {
            white()
        } else if d.month() != vm.month() {
            n300()
        } else if d == today {
            amber600()
        } else if dow == 0 {
            red500()
        } else if dow == 6 {
            blue500()
        } else {
            n700()
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
                    .map(|s| if selected { s.bg(n900()).font_weight(gpui::FontWeight::SEMIBOLD) } else { s.hover(|h| h.bg(n100())) })
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
                            .map(|s| if selected { s.bg(n900()).font_weight(gpui::FontWeight::SEMIBOLD) } else { s.hover(|h| h.bg(n100())) })
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
