//! The bottom strip (like Zed's): page facts on the left; on the right a new notice for a few
//! seconds (click it for the history) and the bell that opens the history, where undo lives.
use crate::app::{HistoryAction, HistoryItem, NoticeHistory, TrackApp, UndoState};
use crate::ui::theme::*;
use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, deferred, div, prelude::FluentBuilder, px};

pub fn render(app: &TrackApp, left: Vec<AnyElement>, cx: &mut Context<TrackApp>) -> impl IntoElement {
    let p = palette(cx);
    let state = app.state.read(cx);
    let history = state.history.clone();
    let flash = state.flash.and_then(|id| history.get(id).cloned());
    let open = app.history_open;
    div()
        .id("status-bar")
        .debug_selector(|| "status-bar".into())
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap_4()
        .relative()
        .h(px(STATUS_BAR_HEIGHT))
        .px_3()
        .bg(p.surface)
        .border_t_1()
        .border_color(p.border)
        .text_size(px(11.))
        .text_color(p.text_muted)
        .children(left)
        .child(div().flex_1())
        .when_some(flash, |d, item| d.child(flash_notice(&item, p, cx)))
        .child(history_bell(open, p, cx))
        .when(open, |d| d.child(history_list(&history, p, cx)))
}

/// About as tall as Zed's status bar (4px above and below 22px buttons).
pub const STATUS_BAR_HEIGHT: f32 = 30.;

fn toggle_history(this: &mut TrackApp, open: bool, cx: &mut Context<TrackApp>) {
    this.history_open = open;
    cx.notify();
}

/// The newest notice for a few seconds; clicking it opens the history.
fn flash_notice(item: &HistoryItem, p: Palette, cx: &mut Context<TrackApp>) -> impl IntoElement {
    let ai = matches!(item.action, Some(HistoryAction::UndoAi(_)));
    div()
        .id("status-flash")
        .debug_selector(|| "status-flash".into())
        .flex()
        .min_w_0()
        .max_w(px(480.))
        .items_center()
        .gap_2()
        .px_1p5()
        .rounded(RADIUS_SM)
        .cursor_pointer()
        .hover(move |s| s.bg(p.hover))
        .when(ai, |d| d.child(div().size(px(6.)).rounded_full().bg(p.warning).flex_none()))
        .child(
            div()
                .when(item.error, |d| d.debug_selector(|| "status-flash-error".into()))
                .min_w_0()
                .truncate()
                .text_color(if item.error { p.danger } else { p.text })
                .child(item.text.clone()),
        )
        .on_click(cx.listener(|this, _, _, cx| toggle_history(this, true, cx)))
}

/// The bell at the right end; it opens the list above it.
fn history_bell(open: bool, p: Palette, cx: &mut Context<TrackApp>) -> impl IntoElement {
    div()
        .id("status-history")
        .debug_selector(|| "status-history".into())
        .relative()
        .flex_none()
        .p(px(3.))
        .rounded(RADIUS_SM)
        .cursor_pointer()
        .text_color(if open { p.text } else { p.text_muted })
        .when(open, |d| d.bg(p.hover))
        .hover(move |s| s.text_color(p.text).bg(p.hover))
        .child(crate::ui::icons::bell().size(px(12.)))
        .on_click(cx.listener(|this, _, _, cx| {
            let open = !this.history_open;
            toggle_history(this, open, cx);
        }))
}

fn history_list(history: &NoticeHistory, p: Palette, cx: &mut Context<TrackApp>) -> impl IntoElement {
    let rows: Vec<_> = history.items.iter().enumerate().map(|(i, item)| history_row(i, item, p, cx)).collect();
    deferred(
        div()
            .id("status-history-list")
            .debug_selector(|| "status-history-list".into())
            // A solid panel standing on the status bar at the window's right edge: square, no
            // shadow, a title strip, rows split by lines.
            .absolute()
            .bottom(px(STATUS_BAR_HEIGHT))
            .right_0()
            .w(px(360.))
            .max_h(px(320.))
            .overflow_y_scroll()
            .border_l_1()
            .border_t_1()
            .border_color(p.border)
            .bg(p.bg)
            .text_size(px(12.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .h(px(28.))
                    .px_3()
                    .bg(p.surface)
                    .border_b_1()
                    .border_color(p.border)
                    .child(div().flex_1().font_weight(gpui::FontWeight::SEMIBOLD).text_color(p.text).child("通知"))
                    .when(!history.items.is_empty(), |d| {
                        d.child(
                        div()
                            .id("status-history-clear")
                            .debug_selector(|| "status-history-clear".into())
                            .px_1p5()
                            .rounded(RADIUS_SM)
                            .text_color(p.text_muted)
                            .cursor_pointer()
                            .hover(move |s| s.text_color(p.text).bg(p.hover))
                            .child("履歴を消す")
                            .on_click(cx.listener(|this, _, _, cx| this.state.update(cx, |s, cx| s.clear_history(cx)))),
                        )
                    }),
            )
            .when(history.items.is_empty(), |d| d.child(div().px_3().py_2().text_color(p.text_muted).child("通知はまだありません")))
            .children(rows)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.history_open = false;
                cx.notify();
            })),
    )
    .with_priority(5)
}

/// A row: the text and, while it can be undone, 「元に戻す」 (or how the undo went).
fn history_row(i: usize, item: &HistoryItem, p: Palette, cx: &mut Context<TrackApp>) -> AnyElement {
    let id = item.id;
    let undo = item.action.as_ref().map(|_| item.undo.clone());
    div()
        .debug_selector(move || format!("status-history-item-{i}"))
        .flex()
        .items_start()
        .gap_2()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(p.border_subtle)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(div().text_color(if item.error { p.danger } else { p.text }).child(item.text.clone()))
                .when_some(match &undo {
                    Some(UndoState::Failed(reason)) => Some(reason.clone()),
                    _ => None,
                }, |d, reason| d.child(div().debug_selector(move || format!("status-history-failed-{i}")).text_color(p.danger).child(reason))),
        )
        .when_some(undo, move |d, state| {
            d.child(match state {
                UndoState::Open | UndoState::Failed(_) => div()
                    .id(("status-history-undo", i))
                    .debug_selector(move || format!("status-history-undo-{i}"))
                    .flex_none()
                    .px_1p5()
                    .rounded(RADIUS_SM)
                    .text_color(p.accent)
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.hover))
                    .child("元に戻す")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(entry) = this.state.update(cx, |s, cx| s.undo_from_history(id, cx)) {
                            this.calendar.update(cx, |c, cx| c.restore_deleted(entry, window, cx));
                        }
                    }))
                    .into_any_element(),
                UndoState::Busy => div().flex_none().px_1p5().text_color(p.text_muted).child("取り消し中…").into_any_element(),
                UndoState::Done => div()
                    .debug_selector(move || format!("status-history-done-{i}"))
                    .flex_none()
                    .px_1p5()
                    .text_color(p.text_muted)
                    .child("元に戻しました")
                    .into_any_element(),
            })
        })
        .into_any_element()
}
