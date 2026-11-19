//! The bottom strip (like Zed's): page facts on the left, notices on the right — the user's own
//! change (a deletion to undo, a failed save) and the AI's.
use crate::app::{AiOpNotice, NoticeHistory, StatusNote, TrackApp};
use crate::ui::theme::*;
use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, deferred, div, prelude::FluentBuilder, px};

pub fn render(app: &TrackApp, left: Vec<AnyElement>, cx: &mut Context<TrackApp>) -> impl IntoElement {
    let p = palette(cx);
    let state = app.state.read(cx);
    let (note, notice, history) = (state.note.clone(), state.ai_op.clone(), state.history.clone());
    let open = app.history_open;
    div()
        .id("status-bar")
        .debug_selector(|| "status-bar".into())
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap_4()
        .h(px(24.))
        .px_3()
        .bg(p.surface)
        .border_t_1()
        .border_color(p.border)
        .text_size(px(11.))
        .text_color(p.text_muted)
        .children(left)
        .child(div().flex_1())
        .when_some(note, |d, n| d.child(own_note(n, p, cx)))
        .when_some(notice, |d, n| d.child(ai_notice(&n, p, cx)))
        .child(history_bell(&history, open, p, cx))
}

/// The bell at the right end: a dot while something is unseen; it opens the list above it.
fn history_bell(history: &NoticeHistory, open: bool, p: Palette, cx: &mut Context<TrackApp>) -> impl IntoElement {
    let bell = div()
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
        .when(history.unseen, |d| {
            d.child(div().debug_selector(|| "status-history-dot".into()).absolute().top(px(1.)).right(px(1.)).size(px(5.)).rounded_full().bg(p.accent))
        })
        .on_click(cx.listener(|this, _, _, cx| {
            this.history_open = !this.history_open;
            if this.history_open {
                this.state.update(cx, |s, cx| s.mark_history_seen(cx));
            }
            cx.notify();
        }));
    div().relative().flex_none().child(bell).when(open, |d| d.child(history_list(history, p, cx)))
}

fn history_list(history: &NoticeHistory, p: Palette, cx: &mut Context<TrackApp>) -> impl IntoElement {
    let rows = history.items.iter().enumerate().map(|(i, item)| {
        div()
            .debug_selector(move || format!("status-history-item-{i}"))
            .flex()
            .gap_2()
            .px_2()
            .py_1()
            .child(div().flex_none().font_family(MONO).text_color(p.text_muted).child(item.at.format("%H:%M").to_string()))
            .child(div().min_w_0().text_color(if item.error { p.danger } else { p.text }).child(item.text.clone()))
    });
    deferred(
        div()
            .id("status-history-list")
            .debug_selector(|| "status-history-list".into())
            .absolute()
            .bottom(px(24.))
            .right_0()
            .w(px(360.))
            .max_h(px(320.))
            .overflow_y_scroll()
            .py_1()
            .rounded_lg()
            .border_1()
            .border_color(p.border)
            .bg(p.elevated)
            .shadow_md()
            .text_size(px(12.))
            .when(history.items.is_empty(), |d| d.child(div().px_2().py_1().text_color(p.text_muted).child("通知はまだありません")))
            .children(rows)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.history_open = false;
                cx.notify();
            })),
    )
    .with_priority(5)
}

/// A link-like action in a notice (「元に戻す」).
fn action(id: &'static str, label: &'static str, p: Palette) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .flex_none()
        .px_1p5()
        .rounded(RADIUS_SM)
        .text_color(p.accent)
        .cursor_pointer()
        .hover(move |s| s.bg(p.hover))
        .child(label)
}

fn close(id: &'static str, p: Palette) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .flex_none()
        .px_1()
        .rounded(RADIUS_SM)
        .cursor_pointer()
        .hover(move |s| s.text_color(p.text).bg(p.hover))
        .child("×")
}

fn own_note(n: StatusNote, p: Palette, cx: &mut Context<TrackApp>) -> impl IntoElement {
    let row = div().id("status-note").debug_selector(|| "status-note".into()).flex().min_w_0().max_w(px(480.)).items_center().gap_2();
    let row = match n {
        StatusNote::Deleted(entry) => {
            row.child(div().min_w_0().truncate().text_color(p.text).child(StatusNote::Deleted(entry.clone()).text()))
                .child(action("status-note-undo", "元に戻す", p).on_click(cx.listener(move |this, _, window, cx| {
                    let entry = entry.clone();
                    this.state.update(cx, |s, cx| s.dismiss_note(cx));
                    this.calendar.update(cx, |c, cx| c.restore_deleted(*entry, window, cx));
                })))
        }
        StatusNote::Error(msg) => row.child(div().debug_selector(|| "status-note-error".into()).min_w_0().truncate().text_color(p.danger).child(msg)),
    };
    row.child(close("status-note-close", p).on_click(cx.listener(|this, _, _, cx| this.state.update(cx, |s, cx| s.dismiss_note(cx)))))
}

fn ai_notice(n: &AiOpNotice, p: Palette, cx: &mut Context<TrackApp>) -> impl IntoElement {
    div()
        .id("ai-op-banner")
        .debug_selector(|| "ai-op-banner".into())
        .flex()
        .min_w_0()
        .max_w(px(640.))
        .items_center()
        .gap_2()
        .child(div().size(px(6.)).rounded_full().bg(p.warning).flex_none())
        .child(div().debug_selector(|| "ai-op-summary".into()).min_w_0().truncate().text_color(p.text).child(format!("AI が {} 件変更しました：{}", n.count(), n.summary())))
        .when_some(n.error.clone(), |d, e| d.child(div().debug_selector(|| "ai-op-error".into()).flex_shrink_0().max_w(px(280.)).truncate().text_color(p.danger).child(e)))
        .child(
            div()
                .id("ai-op-undo")
                .debug_selector(|| "ai-op-undo".into())
                .flex_none()
                .px_1p5()
                .rounded(RADIUS_SM)
                .text_color(p.accent)
                .when(n.busy, |d| d.opacity(0.5))
                .when(!n.busy, move |d| d.cursor_pointer().hover(move |s| s.bg(p.hover)))
                .child(if n.busy { "取り消し中…" } else { "元に戻す" })
                .on_click(cx.listener(|this, _, _, cx| this.state.update(cx, |s, cx| s.undo_ai_op(cx)))),
        )
        .child(
            div()
                .id("ai-op-close")
                .debug_selector(|| "ai-op-close".into())
                .flex_none()
                .px_1()
                .rounded(RADIUS_SM)
                .cursor_pointer()
                .hover(move |s| s.text_color(p.text).bg(p.hover))
                .child("×")
                .on_click(cx.listener(|this, _, _, cx| this.state.update(cx, |s, cx| s.dismiss_ai_op(cx)))),
        )
}
