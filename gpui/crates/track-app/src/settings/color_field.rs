//! The eyedropper swatch next to the preset colours: a circle that opens Hue / Saturation /
//! Lightness sliders and a hex field (no alpha: colours are stored as `#rrggbb`). Shows the
//! picked colour itself when it is not one of the presets.
use crate::ui::theme::*;
use gpui::{App, ElementId, Entity, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled, Window, div, prelude::FluentBuilder, px};
use gpui_component::color_picker::ColorPickerState;
use gpui_component::input::Input;
use gpui_component::popover::Popover;
use gpui_component::slider::Slider;
use gpui_component::{Selectable, h_flex, v_flex};

/// The 24px circle that triggers the popover.
#[derive(IntoElement)]
struct EyedropperSwatch {
    /// The custom colour, when the current colour is not a preset.
    fill: Option<Hsla>,
    palette: Palette,
    selected: bool,
}

impl Selectable for EyedropperSwatch {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

/// Black or white, whichever reads on `c`.
fn ink_on(c: Hsla) -> Hsla {
    let rgb = gpui::Rgba::from(c);
    let lum = 0.2126 * rgb.r + 0.7152 * rgb.g + 0.0722 * rgb.b;
    if lum > 0.55 { gpui::black() } else { gpui::white() }
}

impl RenderOnce for EyedropperSwatch {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let p = self.palette;
        div()
            .id("editor-color-custom")
            .debug_selector(|| "editor-color-custom".into())
            .size(px(24.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .cursor_pointer()
            .map(|d| match self.fill {
                // The picked colour, ringed like a selected preset.
                Some(c) => d.bg(c).border_2().border_color(p.text).child(crate::ui::icons::pipette().size(px(14.)).text_color(ink_on(c))),
                None => d.border_1().border_color(p.border).child(crate::ui::icons::pipette().size(px(14.)).text_color(p.text_muted)),
            })
    }
}

fn slider_row(id: &'static str, label: &'static str, slider: Entity<gpui_component::slider::SliderState>, p: Palette) -> impl IntoElement {
    h_flex()
        .debug_selector(move || id.into())
        .gap_2()
        .items_center()
        .child(div().w(px(36.)).text_xs().text_color(p.text_muted).child(label))
        .child(Slider::new(&slider).flex_1())
}

/// The circle plus its popover. `fill` is the current colour when it is not a preset.
pub fn eyedropper(id: impl Into<ElementId>, picker: &Entity<ColorPickerState>, fill: Option<Hsla>, p: Palette, cx: &App) -> impl IntoElement {
    let state = picker.read(cx);
    let open = state.is_open();
    let sliders = state.sliders();
    let (hue, saturation, lightness) = (sliders.hue().clone(), sliders.saturation().clone(), sliders.lightness().clone());
    let hex = state.hex_input().clone();
    let toggle = picker.clone();
    Popover::new(id)
        .open(open)
        .on_open_change(move |open: &bool, window, cx| {
            let open = *open;
            toggle.update(cx, |s, cx| {
                if open {
                    s.sync_pending_value(window, cx);
                }
                s.set_open(open, cx);
            });
        })
        .trigger(EyedropperSwatch { fill, palette: p, selected: false })
        .when(open, |pop| {
            pop.w(px(260.)).child(
                v_flex()
                    .gap_2()
                    .p_1()
                    .child(slider_row("picker-hue", "色相", hue, p))
                    .child(slider_row("picker-saturation", "彩度", saturation, p))
                    .child(slider_row("picker-lightness", "明度", lightness, p))
                    .child(div().debug_selector(|| "picker-hex".into()).child(Input::new(&hex))),
            )
        })
}
