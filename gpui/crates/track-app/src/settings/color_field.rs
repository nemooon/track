//! The eyedropper swatch next to the preset colours: a circle that opens Hue / Saturation /
//! Lightness sliders and a hex field (no alpha: colours are stored as `#rrggbb`). Shows the
//! picked colour itself when it is not one of the presets.
use crate::ui::theme::*;
use gpui::{App, ElementId, Entity, Hsla, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled, Window, div, hsla, linear_color_stop, linear_gradient, prelude::FluentBuilder, px};
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

/// A strip of left-to-right gradients through `stops`, drawn behind a slider.
fn track(id: &'static str, stops: Vec<Hsla>) -> impl IntoElement {
    let segments = stops.windows(2).map(|w| div().flex_1().h_full().bg(linear_gradient(90., linear_color_stop(w[0], 0.), linear_color_stop(w[1], 1.)))).collect::<Vec<_>>();
    h_flex()
        .debug_selector(move || id.into())
        .absolute()
        .left_0()
        .right_0()
        .h(px(8.))
        .rounded_full()
        .overflow_hidden()
        .children(segments)
}

fn slider_row(id: &'static str, label: &'static str, slider: Entity<gpui_component::slider::SliderState>, stops: Vec<Hsla>, p: Palette) -> impl IntoElement {
    let track_id: &'static str = match id {
        "picker-hue" => "picker-hue-track",
        "picker-saturation" => "picker-saturation-track",
        _ => "picker-lightness-track",
    };
    h_flex()
        .debug_selector(move || id.into())
        .gap_2()
        .items_center()
        .child(div().w(px(36.)).text_xs().text_color(p.text_muted).child(label))
        .child(
            div()
                .relative()
                .flex()
                .items_center()
                .flex_1()
                .h(px(24.))
                .child(track(track_id, stops))
                .child(Slider::new(&slider).flex_1().bg(gpui::transparent_black())),
        )
}

/// The circle plus its popover. `fill` is the current colour when it is not a preset.
pub fn eyedropper(id: impl Into<ElementId>, picker: &Entity<ColorPickerState>, fill: Option<Hsla>, p: Palette, cx: &App) -> impl IntoElement {
    let state = picker.read(cx);
    let open = state.is_open();
    let sliders = state.sliders();
    let (hue, saturation, lightness) = (sliders.hue().clone(), sliders.saturation().clone(), sliders.lightness().clone());
    let hex = state.hex_input().clone();
    // The tracks follow the colour being picked.
    let c = state.displayed_color().unwrap_or(hsla(0., 0., 0.5, 1.));
    let hue_stops: Vec<Hsla> = (0..=6).map(|i| hsla(i as f32 / 6., 0.85, 0.55, 1.)).collect();
    let sat_stops = vec![hsla(c.h, 0., c.l, 1.), hsla(c.h, 1., c.l, 1.)];
    let light_stops = vec![hsla(c.h, c.s, 0., 1.), hsla(c.h, c.s, 0.5, 1.), hsla(c.h, c.s, 1., 1.)];
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
                    .child(slider_row("picker-hue", "色相", hue, hue_stops, p))
                    .child(slider_row("picker-saturation", "彩度", saturation, sat_stops, p))
                    .child(slider_row("picker-lightness", "明度", lightness, light_stops, p))
                    .child(div().debug_selector(|| "picker-hex".into()).child(Input::new(&hex))),
            )
        })
}
