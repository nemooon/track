//! The day header's 休暇・予定 menu and mark. The menu is a popover from a small button: leave
//! (全日・前半・後半・なし, 「休暇にしない」 on a holiday, saved at once) and 予定: 「終日」 frames the
//! day's work hours and opens the picker new entries use (or the day's 終日 with 「外す」). Leave
//! and a 終日 never share a day (core keeps that).
use super::CalendarView;
use crate::core::holidays::holiday_name;
use crate::settings::popover::Trigger;
use crate::ui::{controls, theme::*};
use chrono::{Datelike, NaiveDate};
use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px};
use gpui_component::popover::Popover;
use std::future::Future;
use track_core::model::{EntryKind, LeaveKind, TimeEntry};

impl CalendarView {
    fn direct_on(&self, d: NaiveDate) -> Option<&TimeEntry> {
        self.cache.entries().iter().find(|e| e.kind == EntryKind::Direct && e.start.with_timezone(&chrono::Local).date_naive() == d)
    }

    /// A holiday on a work day is full leave unless a row says otherwise.
    fn auto_leave(&self, d: NaiveDate, cx: &gpui::App) -> bool {
        let work_day = self.state.read(cx).data.as_ref().is_some_and(|data| data.settings.work_days.contains(&d.weekday().num_days_from_sunday()));
        work_day && holiday_name(d).is_some()
    }

    /// The holiday's automatic leave applies (no row cancels it, no direct visit that day).
    pub(super) fn counts_holiday_leave(&self, d: NaiveDate, cx: &gpui::App) -> bool {
        self.auto_leave(d, cx) && !self.leaves.contains_key(&d) && self.direct_on(d).is_none()
    }

    /// 「休暇」「前半休」「後半休」 or 「終日 プロジェクト」 in the header.
    pub(super) fn day_mark(&self, d: NaiveDate, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = palette(cx);
        let sel = format!("day-mark-{d}");
        let badge = |text: String, bg: gpui::Hsla, fg: gpui::Hsla| {
            div().debug_selector(move || sel.clone()).min_w_0().rounded(RADIUS_SM).bg(bg).px_1().py(px(2.)).text_size(px(10.)).line_height(px(12.)).text_color(fg).truncate().child(text)
        };
        if let Some(e) = self.direct_on(d) {
            let (name, color) = e.project.as_ref().map(|p| (p.name.clone(), p.color.clone())).unwrap_or_default();
            let c = hex(if color.is_empty() { NO_PROJECT } else { &color });
            let text = if name.is_empty() { "終日".to_string() } else { format!("終日 {name}") };
            return Some(badge(text, gpui::Hsla::from(mix(c, 0.18, p.bg)), p.text).into_any_element());
        }
        let label = match self.leaves.get(&d) {
            Some(LeaveKind::Full) => "休暇",
            Some(LeaveKind::Am) => "前半休",
            Some(LeaveKind::Pm) => "後半休",
            _ => return None,
        };
        Some(badge(label.into(), with_alpha(p.heat, 0.18), p.text).into_any_element())
    }

    /// Saves through `fut`, then re-reads everything (this page and the reports).
    pub(super) fn save_day(&mut self, fut: impl Future<Output = crate::data::DataResult<impl Send + 'static>> + Send + 'static, cx: &mut Context<Self>) {
        self.day_menu = None;
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                if let Err(e) = result {
                    this.load_error = Some(e.message);
                }
                this.state.update(cx, |s, cx| s.refresh_all(cx));
                this.reload(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(super) fn render_day_menu(&self, d: NaiveDate, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let open = self.day_menu == Some(d);
        let button = controls::icon_button(&p, SharedString::from(format!("day-menu-{d}")), crate::ui::icons::calendar_cog(), "休暇・予定")
            .debug_selector(move || format!("day-menu-{d}"));
        let this = cx.entity().downgrade();
        let mut pop = Popover::new(SharedString::from(format!("day-menu-pop-{d}")))
            .open(open)
            .on_open_change(move |open: &bool, _, cx| {
                let open = *open;
                let _ = this.update(cx, |this, cx| {
                    this.day_menu = open.then_some(d);
                    cx.notify();
                });
            })
            .trigger(Trigger::new(button));
        if open {
            pop = pop.p_1().child(self.menu_content(d, p, cx));
        }
        pop.into_any_element()
    }

    /// A narrow list like a Mac menu: the leave kinds (a check on the current one), a line, then
    /// 「終日の予定を入れる…」 or the day's 終日 with 「外す」.
    fn menu_content(&self, d: NaiveDate, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.leaves.get(&d).copied();
        let holiday = self.auto_leave(d, cx);
        let direct = self.direct_on(d).cloned();
        let row = |id: &'static str| {
            div()
                .id(id)
                .debug_selector(move || id.into())
                .flex()
                .items_center()
                .gap_2()
                .h(px(26.))
                .px_2()
                .rounded(RADIUS_SM)
                .text_size(px(13.))
                .text_color(p.text)
                .cursor_pointer()
                .hover(move |s| s.bg(p.hover))
        };
        let mark = |id: &'static str, on: bool| {
            div().w(px(14.)).flex_none().text_color(p.accent).when(on, |d| d.debug_selector(move || format!("{id}-check")).child(gpui_component::Icon::new(gpui_component::IconName::Check).size(px(13.))))
        };
        let leave_row = |id: &'static str, label: &'static str, kind: Option<LeaveKind>, on: bool, cx: &mut Context<Self>| {
            row(id).child(mark(id, on)).child(label).on_click(cx.listener(move |this, _, _, cx| {
                let fut = this.state.read(cx).api.set_leave(d, kind);
                this.save_day(fut, cx);
            }))
        };
        let none_label = if holiday { "休暇にしない" } else { "なし" };
        let none_on = direct.is_none()
            && match current {
                Some(LeaveKind::None) => true,
                None => !holiday,
                _ => false,
            };
        let full_on = direct.is_none() && (current == Some(LeaveKind::Full) || (current.is_none() && holiday));
        div()
            .debug_selector(|| "day-menu-list".into())
            .w(px(MENU_WIDTH))
            .flex()
            .flex_col()
            .child(div().px_2().pt_1().pb(px(2.)).text_size(px(11.)).text_color(p.text_muted).child("休暇"))
            .child(leave_row("day-leave-none", none_label, None, none_on, cx))
            .child(leave_row("day-leave-full", "休暇（1 日）", Some(LeaveKind::Full), full_on, cx))
            .child(leave_row("day-leave-am", "前半休", Some(LeaveKind::Am), current == Some(LeaveKind::Am), cx))
            .child(leave_row("day-leave-pm", "後半休", Some(LeaveKind::Pm), current == Some(LeaveKind::Pm), cx))
            .child(div().my_1().h(px(1.)).bg(p.border_subtle))
            .map(|c| match direct {
                None => c.child(
                    row("day-allday")
                        .child(div().w(px(14.)).flex_none().text_color(p.text_muted).child(gpui_component::Icon::new(gpui_component::IconName::Plus).size(px(13.))))
                        .child("終日の予定を入れる…")
                        .on_click(cx.listener(move |this, _, window, cx| this.start_all_day(d, window, cx))),
                ),
                Some(e) => {
                    let (name, color) = e.project.as_ref().map(|p| (p.name.clone(), p.color.clone())).unwrap_or_else(|| ("プロジェクトなし".into(), String::new()));
                    c.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .h(px(26.))
                            .px_2()
                            .text_size(px(13.))
                            .child(div().w(px(14.)).flex_none().flex().justify_center().child(div().size(px(8.)).rounded_full().bg(hex(if color.is_empty() { NO_PROJECT } else { &color }))))
                            .child(div().flex_1().min_w_0().truncate().child(format!("終日：{name}")))
                            .child(
                                div()
                                    .id("day-direct-remove")
                                    .debug_selector(|| "day-direct-remove".into())
                                    .flex_none()
                                    .px_1p5()
                                    .rounded(RADIUS_SM)
                                    .cursor_pointer()
                                    .text_size(px(12.))
                                    .text_color(p.danger)
                                    .hover(move |s| s.bg(p.hover))
                                    .child("外す")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let fut = this.state.read(cx).api.remove_direct(d);
                                        this.save_day(fut, cx);
                                    })),
                            ),
                    )
                }
            })
    }
}

/// The day menu's width: enough for 「終日の予定を入れる…」 and a project name.
const MENU_WIDTH: f32 = 216.;
