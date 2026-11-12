//! `ToolbarControlGroup` / `ToolbarControlButton` from ToolbarControls.tsx.
use super::theme::*;
use gpui::{Div, ElementId, InteractiveElement, Stateful, Styled, div, prelude::FluentBuilder, px};

pub fn toolbar() -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .min_h(px(48.))
        .flex_shrink_0()
        .px_3()
        .py_2()
        .bg(white())
        .border_b_1()
        .border_color(n200())
}

pub fn group() -> Div {
    div().flex().flex_shrink_0().items_center().gap(px(2.)).rounded_lg().bg(n100()).p(px(2.))
}

pub fn button(id: impl Into<ElementId>, active: bool, icon_only: bool) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .h(px(32.))
        .rounded_md()
        .text_xs()
        .font_weight(gpui::FontWeight::MEDIUM)
        .cursor_pointer()
        .map(|d| if icon_only { d.w(px(32.)) } else { d.px(px(10.)) })
        .map(|d| {
            if active {
                d.bg(white()).text_color(n900()).shadow_sm().border_1().border_color(black_a(0.05))
            } else {
                d.text_color(n500()).hover(|s| s.bg(white_a(0.7)).text_color(n900()))
            }
        })
}

pub fn disabled(d: Stateful<Div>, is_disabled: bool) -> Stateful<Div> {
    if is_disabled { d.opacity(0.4).cursor_default() } else { d }
}
