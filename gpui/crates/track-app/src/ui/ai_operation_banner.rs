//! 「AI が N 件変更しました［元に戻す］」: offered after an AI (track-cli) operation lands.
use crate::app::{AiOpNotice, TrackApp};
use crate::ui::theme::*;
use gpui::{Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px};

pub fn render(notice: Option<AiOpNotice>, cx: &mut Context<TrackApp>) -> Option<impl IntoElement> {
    let n = notice?;
    let op = &n.operation;
    Some(
        div()
            .id("ai-op-banner")
            .debug_selector(|| "ai-op-banner".into())
            .flex()
            .flex_none()
            .items_center()
            .gap_3()
            .px_4()
            .py(px(6.))
            .bg(amber50())
            .border_b_1()
            .border_color(amber100())
            .text_xs()
            .text_color(n800())
            .child(div().flex_1().min_w_0().truncate().child(format!("AI が {} 件変更しました：{}", op.count, op.summary)))
            .when_some(n.error.clone(), |d, e| d.child(div().debug_selector(|| "ai-op-error".into()).flex_none().text_color(red700()).child(e)))
            .child(
                div()
                    .id("ai-op-undo")
                    .debug_selector(|| "ai-op-undo".into())
                    .flex_none()
                    .px_2()
                    .py(px(2.))
                    .rounded_md()
                    .border_1()
                    .border_color(amber500())
                    .bg(white())
                    .when(n.busy, |d| d.opacity(0.5))
                    .when(!n.busy, |d| d.cursor_pointer().hover(|s| s.bg(amber100())))
                    .child(if n.busy { "取り消し中…" } else { "元に戻す" })
                    .on_click(cx.listener(|this, _, _, cx| this.state.update(cx, |s, cx| s.undo_ai_op(cx)))),
            )
            .child(
                div()
                    .id("ai-op-close")
                    .debug_selector(|| "ai-op-close".into())
                    .flex_none()
                    .px_1()
                    .text_color(n500())
                    .cursor_pointer()
                    .hover(|s| s.text_color(n800()))
                    .child("×")
                    .on_click(cx.listener(|this, _, _, cx| this.state.update(cx, |s, cx| s.dismiss_ai_op(cx)))),
            ),
    )
}
