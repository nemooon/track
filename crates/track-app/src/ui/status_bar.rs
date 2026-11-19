//! The bottom strip (like Zed's): page facts on the left, notices on the right — the user's own
//! change (a deletion to undo, a failed save) and the AI's.
use crate::app::{AiOpNotice, StatusNote, TrackApp};
use crate::ui::theme::*;
use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px};

pub fn render(left: Vec<AnyElement>, note: Option<StatusNote>, notice: Option<AiOpNotice>, cx: &mut Context<TrackApp>) -> impl IntoElement {
    let p = palette(cx);
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
            let title = entry.title.clone().filter(|t| !t.is_empty()).unwrap_or_else(|| "タイトルなし".into());
            row.child(div().min_w_0().truncate().text_color(p.text).child(format!("1件削除しました：{title}")))
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
