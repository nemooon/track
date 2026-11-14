//! Things that open over the page from the control that asked for them (a confirmation from a
//! delete icon, colours from a dot), instead of panels pushed in under a row.
use crate::ui::theme::*;
use gpui::{AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px};
use gpui_component::popover::Popover;
use gpui_component::{Selectable, v_flex};
use std::rc::Rc;

/// Any element as a popover trigger.
#[derive(IntoElement)]
pub struct Trigger {
    child: AnyElement,
    selected: bool,
}

impl Trigger {
    pub fn new(child: impl IntoElement) -> Self {
        Trigger { child: child.into_any_element(), selected: false }
    }
}

impl Selectable for Trigger {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for Trigger {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.child
    }
}

/// The words and button ids of a confirmation.
pub struct Confirm {
    pub message: String,
    pub cancel_id: &'static str,
    pub confirm_id: &'static str,
    pub confirm_label: &'static str,
}

/// A popover on `trigger` asking to confirm. The caller owns `open`; `set_open` keeps it in step
/// (clicking the trigger, clicking outside, キャンセル). `on_confirm` runs on the confirm button.
pub fn confirm(
    id: impl Into<ElementId>,
    trigger: impl IntoElement,
    open: bool,
    set_open: impl Fn(bool, &mut Window, &mut App) + 'static,
    c: Confirm,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    p: Palette,
) -> Popover {
    let set_open = Rc::new(set_open);
    let (on_change, on_cancel) = (set_open.clone(), set_open);
    let on_confirm = Rc::new(on_confirm);
    Popover::new(id)
        .open(open)
        .on_open_change(move |open: &bool, window, cx| on_change(*open, window, cx))
        .trigger(Trigger::new(trigger))
        .when(open, |pop| {
            pop.w(px(280.)).child(
                v_flex()
                    .gap_3()
                    .p_1()
                    .child(div().text_xs().text_color(p.text).child(c.message))
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(super::form::button(p, c.cancel_id, "キャンセル", false, true).on_click(move |_, window, cx| on_cancel(false, window, cx)))
                            .child(super::form::button(p, c.confirm_id, c.confirm_label, true, true).on_click(move |_, window, cx| on_confirm(window, cx))),
                    ),
            )
        })
}
