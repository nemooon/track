//! The bottom strip (like Zed's): page facts on the left, the AI notice on the right.
use crate::app::{AiOpNotice, TrackApp};
use crate::ui::theme::*;
use gpui::{AnyElement, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px};

pub fn render(left: Vec<AnyElement>, notice: Option<AiOpNotice>, cx: &mut Context<TrackApp>) -> impl IntoElement {
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
        .when_some(notice, |d, n| d.child(ai_notice(&n, p, cx)))
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
