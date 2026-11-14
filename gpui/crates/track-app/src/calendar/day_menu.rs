//! The day header's 休暇・直行直帰 menu and mark. The menu is a popover from a small button: leave
//! (全日・前半・後半・なし, 「休暇にしない」 on a holiday) and 直行直帰 (pick a project, or take it
//! away; the project list and its search are the ones new entries use). Choosing saves at once; leave and a direct visit never share a day (core keeps that).
use super::CalendarView;
use crate::core::holidays::holiday_name;
use crate::settings::popover::Trigger;
use crate::ui::{controls, theme::*};
use chrono::{Datelike, NaiveDate};
use gpui::{AnyElement, AppContext, Context, InteractiveElement, IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::popover::Popover;
use super::picker::{PickFn, project_list};
use std::rc::Rc;
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

    /// 「休暇」「前半休」「後半休」 or 「直行直帰 プロジェクト」 in the header.
    pub(super) fn day_mark(&self, d: NaiveDate, cx: &mut Context<Self>) -> Option<AnyElement> {
        let p = palette(cx);
        let sel = format!("day-mark-{d}");
        let badge = |text: String, bg: gpui::Hsla, fg: gpui::Hsla| {
            div().debug_selector(move || sel.clone()).min_w_0().rounded(RADIUS_SM).bg(bg).px_1().py(px(2.)).text_size(px(10.)).line_height(px(12.)).text_color(fg).truncate().child(text)
        };
        if let Some(e) = self.direct_on(d) {
            let (name, color) = e.project.as_ref().map(|p| (p.name.clone(), p.color.clone())).unwrap_or_default();
            let c = hex(if color.is_empty() { NO_PROJECT } else { &color });
            return Some(badge(format!("直行直帰 {name}"), gpui::Hsla::from(mix(c, 0.18, p.bg)), p.text).into_any_element());
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
    fn save_day(&mut self, fut: impl Future<Output = crate::data::DataResult<impl Send + 'static>> + Send + 'static, cx: &mut Context<Self>) {
        self.day_menu = None;
        self.direct_query = None;
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
        let button = controls::icon_button(&p, SharedString::from(format!("day-menu-{d}")), crate::ui::icons::calendar_cog(), "休暇・直行直帰")
            .debug_selector(move || format!("day-menu-{d}"));
        let this = cx.entity().downgrade();
        let mut pop = Popover::new(SharedString::from(format!("day-menu-pop-{d}")))
            .open(open)
            .on_open_change(move |open: &bool, window, cx| {
                let open = *open;
                let _ = this.update(cx, |this, cx| {
                    this.day_menu = open.then_some(d);
                    this.direct_query = open.then(|| {
                        let q = cx.new(|cx| InputState::new(window, cx).placeholder("プロジェクトを検索"));
                        cx.subscribe(&q, |_, _, _: &InputEvent, cx| cx.notify()).detach();
                        q.update(cx, |s, cx| s.focus(window, cx));
                        q
                    });
                    cx.notify();
                });
            })
            .trigger(Trigger::new(button));
        if open {
            pop = pop.w(px(super::PICKER_WIDTH)).child(self.menu_content(d, p, cx));
        }
        pop.into_any_element()
    }

    fn menu_content(&self, d: NaiveDate, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let current = self.leaves.get(&d).copied();
        let holiday = self.auto_leave(d, cx);
        let direct = self.direct_on(d).cloned();
        let heading = |t: &'static str| div().text_xs().font_weight(gpui::FontWeight::SEMIBOLD).text_color(p.text_muted).child(t);
        let leave_chip = |id: &'static str, label: &'static str, kind: Option<LeaveKind>, on: bool, cx: &mut Context<Self>| {
            controls::chip(&p, id, on).debug_selector(move || id.into()).child(label).on_click(cx.listener(move |this, _, _, cx| {
                let fut = this.state.read(cx).api.set_leave(d, kind);
                this.save_day(fut, cx);
            }))
        };
        let none_label = if holiday { "休暇にしない" } else { "なし" };
        let none_on = direct.is_none() && match current {
            Some(LeaveKind::None) => true,
            None => !holiday,
            _ => false,
        };
        let projects: Vec<track_core::model::Project> = self.state.read(cx).data.as_ref().map(|data| data.projects.clone()).unwrap_or_default();
        let query = self.direct_query.clone();
        let q = query.as_ref().map(|q| q.read(cx).value().to_string()).unwrap_or_default();
        let on_pick: PickFn<Self> = Rc::new(move |this, id, _, cx| {
            let Some(id) = id else { return };
            let fut = this.state.read(cx).api.set_direct(d, id);
            this.save_day(fut, cx);
        });
        let list = project_list(&projects, &q, false, on_pick, cx);
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_1()
            .child(heading("休暇"))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(leave_chip("day-leave-full", "全日", Some(LeaveKind::Full), direct.is_none() && (current == Some(LeaveKind::Full) || (current.is_none() && holiday)), cx))
                    .child(leave_chip("day-leave-am", "前半", Some(LeaveKind::Am), current == Some(LeaveKind::Am), cx))
                    .child(leave_chip("day-leave-pm", "後半", Some(LeaveKind::Pm), current == Some(LeaveKind::Pm), cx))
                    .child(leave_chip("day-leave-none", none_label, None, none_on, cx)),
            )
            .child(heading("直行直帰"))
            .when_some(direct, |c, e| {
                let (name, color) = e.project.as_ref().map(|p| (format!("{} · {}", p.client.name, p.name), p.color.clone())).unwrap_or_default();
                c.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_sm()
                        .child(div().size(px(10.)).flex_none().rounded_full().bg(hex(if color.is_empty() { NO_PROJECT } else { &color })))
                        .child(div().flex_1().min_w_0().truncate().child(name))
                        .child(
                            div()
                                .id("day-direct-remove")
                                .debug_selector(|| "day-direct-remove".into())
                                .flex_none()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .cursor_pointer()
                                .text_xs()
                                .text_color(p.danger)
                                .hover(move |s| s.bg(p.hover))
                                .child("外す")
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let fut = this.state.read(cx).api.remove_direct(d);
                                    this.save_day(fut, cx);
                                })),
                        ),
                )
            })
            .child(
                div()
                    .rounded_md()
                    .border_1()
                    .border_color(p.border)
                    .overflow_hidden()
                    .when_some(query, |c, q| c.child(div().border_b_1().border_color(p.border).child(Input::new(&q).appearance(false).px_3().py_2())))
                    .child(list),
            )
    }
}
