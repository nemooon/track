//! `ToolbarControlGroup` / `ToolbarControlButton` from ToolbarControls.tsx.
use super::theme::*;
use gpui::{Div, ElementId, InteractiveElement, ParentElement, Stateful, Styled, div, prelude::FluentBuilder, px};

pub fn toolbar(p: &Palette) -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .min_h(px(48.))
        .flex_shrink_0()
        .px_3()
        .py_2()
        .bg(p.bg)
        .border_b_1()
        .border_color(p.border)
}

pub fn group(p: &Palette) -> Div {
    div().flex().flex_shrink_0().items_center().gap(px(2.)).rounded_md().bg(p.surface).border_1().border_color(p.border_subtle).p(px(2.))
}

pub fn button(p: &Palette, id: impl Into<ElementId>, active: bool, icon_only: bool) -> Stateful<Div> {
    let p = *p;
    div()
        .id(id)
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .h(px(28.))
        .rounded(px(5.))
        .text_xs()
        .font_weight(gpui::FontWeight::MEDIUM)
        .cursor_pointer()
        .map(|d| if icon_only { d.w(px(32.)) } else { d.px(px(10.)) })
        .map(|d| {
            if active {
                d.bg(p.bg).text_color(p.text).border_1().border_color(p.border)
            } else {
                d.text_color(p.text_muted).hover(move |s| s.bg(p.hover).text_color(p.text))
            }
        })
}

/// 「サーバーに接続できません」 strip with a 再試行 button (id `retry_id`).
pub fn error_banner(p: &Palette, retry_id: &'static str, message: &str) -> Stateful<Div> {
    div()
        .id(retry_id)
        .debug_selector(move || retry_id.into())
        .flex()
        .flex_shrink_0()
        .items_center()
        .gap_3()
        .px_3()
        .py_2()
        .bg(p.danger_bg)
        .border_b_1()
        .border_color(p.danger_border)
        .text_xs()
        .text_color(p.danger)
        .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child("データを読み込めません"))
        .child(div().flex_1().truncate().child(message.to_string()))
        .child(div().rounded_md().bg(p.primary).text_color(p.on_color).px_3().py_1().cursor_pointer().hover(move |s| s.bg(p.primary_hover)).child("再試行"))
}

#[allow(dead_code)]
pub fn disabled(d: Stateful<Div>, is_disabled: bool) -> Stateful<Div> {
    if is_disabled { d.opacity(0.4).cursor_default() } else { d }
}
