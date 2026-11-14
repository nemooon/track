//! Building blocks shared by the settings sections (SettingsCard / SettingsRow / SettingsActions).
use crate::ui::theme::*;
use gpui::{AnyElement, Div, ElementId, FontWeight, InteractiveElement, IntoElement, ParentElement, Stateful, Styled, div, prelude::FluentBuilder, px};

/// A section: a heading area (title, description) followed by its form area. Sections after the
/// first are separated by a border instead of being boxed in cards.
pub fn section(p: Palette, id: &'static str, title: &str, description: &str, first: bool) -> Div {
    let heading = format!("{id}-heading");
    div()
        .debug_selector(move || id.into())
        .flex()
        .flex_col()
        .gap_5()
        .pb_6()
        .when(!first, |d| d.pt_6().border_t_1().border_color(p.border))
        .child(
            div()
                .debug_selector(move || heading.clone())
                .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child(title.to_string()))
                .when(!description.is_empty(), |d| d.child(div().mt_1().text_xs().text_color(p.text_muted).child(description.to_string()))),
        )
}

/// A field in a section: the label and its help above the control.
pub fn field(p: Palette, id: &'static str, title: &str, description: &str, control: impl IntoElement) -> Div {
    let label = format!("{id}-label");
    div()
        .debug_selector(move || id.into())
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .debug_selector(move || label.clone())
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(title.to_string()))
                .when(!description.is_empty(), |d| d.child(div().mt(px(2.)).text_xs().text_color(p.text_muted).child(description.to_string()))),
        )
        .child(control)
}

/// A titled card with a short description.
pub fn card(p: Palette, title: &str, description: &str) -> Div {
    div()
        .flex()
        .flex_col()
        .mb_5()
        .rounded_lg()
        .border_1()
        .border_color(p.border)
        .child(
            div()
                .px_5()
                .py_3()
                .border_b_1()
                .border_color(p.border_subtle)
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child(title.to_string()))
                .when(!description.is_empty(), |d| d.child(div().mt(px(2.)).text_xs().text_color(p.text_muted).child(description.to_string()))),
        )
}

/// A row inside a card: label and help on the left, the control on the right.
pub fn row(p: Palette, title: &str, description: &str, control: impl IntoElement) -> Div {
    div()
        .flex()
        .items_start()
        .gap_6()
        .px_5()
        .py_4()
        .child(
            div()
                .w(px(180.))
                .flex_shrink_0()
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(title.to_string()))
                .when(!description.is_empty(), |d| d.child(div().mt(px(2.)).text_xs().text_color(p.text_muted).child(description.to_string()))),
        )
        .child(div().flex_1().min_w_0().child(control))
}

pub fn button(p: Palette, id: impl Into<ElementId>, label: &str, primary: bool, enabled: bool) -> Stateful<Div> {
    frame(p, id, primary, enabled).child(label.to_string())
}

/// A button with an icon before its label.
pub fn button_with_icon(p: Palette, id: impl Into<ElementId>, icon: gpui_component::Icon, label: &str, primary: bool, enabled: bool) -> Stateful<Div> {
    use gpui_component::Sizable;
    frame(p, id, primary, enabled).gap_1p5().child(icon.small()).child(label.to_string())
}

fn frame(p: Palette, id: impl Into<ElementId>, primary: bool, enabled: bool) -> Stateful<Div> {
    let id = id.into();
    let sel = format!("{id}");
    div()
        .id(id)
        .debug_selector(move || sel.clone())
        .flex()
        .items_center()
        .gap_1()
        .px_3()
        .py(px(6.))
        .rounded_md()
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .when(primary, |d| d.bg(p.primary).text_color(p.on_color))
        .when(!primary, |d| d.border_1().border_color(p.border).text_color(p.text))
        .when(enabled, |d| d.cursor_pointer().hover(|s| s.opacity(0.85)))
        .when(!enabled, |d| d.opacity(0.4))
}

/// The bottom bar: 「保存済み」 when clean, otherwise 変更を破棄 / 変更を保存, plus any error.
pub struct Actions<'a> {
    pub prefix: &'static str,
    pub dirty: bool,
    pub saving: bool,
    pub error: Option<&'a str>,
}

pub fn actions(p: Palette, a: Actions, discard: impl IntoElement, save: impl IntoElement) -> AnyElement {
    let saved_id = format!("{}-saved", a.prefix);
    let error_id = format!("{}-error", a.prefix);
    div()
        .flex()
        .items_center()
        .justify_end()
        .gap_3()
        .mb_8()
        .when_some(a.error.map(str::to_string), |d, e| d.child(div().debug_selector(move || error_id.clone()).mr_auto().text_xs().text_color(p.danger).child(e)))
        .when(!a.dirty && a.error.is_none(), |d| d.child(div().debug_selector(move || saved_id.clone()).text_xs().text_color(p.text_muted).child("保存済み")))
        .when(a.dirty, |d| d.child(discard))
        .when(a.dirty || a.saving, |d| d.child(save))
        .into_any_element()
}

pub fn minutes_label(m: i64) -> String {
    format!("{:02}:{:02}", m / 60, m % 60)
}
