//! Colours by meaning (`Palette`) plus helpers for project colours.
use gpui::{Hsla, Rgba, rgb};

pub fn hex(s: &str) -> Rgba {
    let s = s.trim_start_matches('#');
    let v = u32::from_str_radix(s, 16).unwrap_or(0x9ca3af);
    rgb(if s.len() == 3 {
        let (r, g, b) = ((v >> 8) & 0xf, (v >> 4) & 0xf, v & 0xf);
        (r * 17) << 16 | (g * 17) << 8 | (b * 17)
    } else {
        v
    })
}

/// Linear sRGB mix like CSS `color-mix(in srgb, a p%, b)`.
pub fn mix(a: impl Into<Rgba>, p: f32, b: impl Into<Rgba>) -> Rgba {
    let (a, b): (Rgba, Rgba) = (a.into(), b.into());
    Rgba { r: a.r * p + b.r * (1. - p), g: a.g * p + b.g * (1. - p), b: a.b * p + b.b * (1. - p), a: a.a * p + b.a * (1. - p) }
}

pub fn with_alpha(c: impl Into<Rgba>, a: f32) -> Hsla {
    Rgba { a, ..c.into() }.into()
}

pub const NO_PROJECT: &str = "#9ca3af";

/// Numbers (durations, shares, clock times) use a fixed-width face, like Zed.
pub const MONO: &str = "Menlo";

/// Colours by meaning. Screens read these instead of raw shades, so a dark palette is just
/// another set of values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    /// Body background.
    pub bg: Hsla,
    /// Title bar, status bar and toolbars.
    pub surface: Hsla,
    /// Popovers and dialogs.
    pub elevated: Hsla,
    pub border: Hsla,
    /// Table rows and grid lines.
    pub border_subtle: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    /// Hour labels and other quiet hints.
    pub text_faint: Hsla,
    pub hover: Hsla,
    /// The selected tab, button or row.
    pub selected: Hsla,
    /// Today, links, focus.
    pub accent: Hsla,
    pub accent_text: Hsla,
    pub danger: Hsla,
    pub danger_bg: Hsla,
    pub warning_bg: Hsla,
    pub now_line: Hsla,
    /// Calendar inside working hours (outside is `off_hours`).
    pub work_hours: Hsla,
    pub off_hours: Hsla,
    /// Sunday / Saturday / holiday day names and numbers.
    pub sunday: Hsla,
    pub saturday: Hsla,
    pub holiday: Hsla,
    /// Drop shadows (used with `with_alpha`).
    pub shadow: Hsla,
}

impl gpui::Global for Palette {}

fn c(v: u32) -> Hsla {
    rgb(v).into()
}

impl Palette {
    /// Light values in the spirit of Zed's One Light.
    pub fn light() -> Self {
        Palette {
            bg: c(0xffffff),
            surface: c(0xf4f4f5),
            elevated: c(0xffffff),
            border: c(0xdedee1),
            border_subtle: c(0xececee),
            text: c(0x1f2328),
            text_muted: c(0x5f6368),
            text_faint: c(0x9a9da3),
            hover: c(0xebebed),
            selected: c(0xe2e4e8),
            accent: c(0x2563eb),
            accent_text: c(0xffffff),
            danger: c(0xdc2626),
            danger_bg: c(0xfef2f2),
            warning_bg: c(0xfffbeb),
            now_line: c(0xef4444),
            work_hours: c(0xffffff),
            off_hours: c(0xf7f7f8),
            sunday: c(0xdc2626),
            saturday: c(0x2563eb),
            holiday: c(0xd97706),
            shadow: c(0x000000),
        }
    }
}

/// The palette in use (light until a dark one exists).
pub fn palette(cx: &gpui::App) -> Palette {
    cx.try_global::<Palette>().copied().unwrap_or_else(Palette::light)
}

/// Installs the light palette for every window and pins gpui-component's widgets to light
/// with the same base colours, whatever the macOS appearance.
pub fn install_light_theme(cx: &mut gpui::App) {
    let p = Palette::light();
    cx.set_global(p);
    gpui_component::Theme::change(gpui_component::ThemeMode::Light, None, cx);
    gpui_component::Theme::update(cx, |t| {
        t.background = p.bg;
        t.foreground = p.text;
        t.border = p.border;
        t.input = p.border;
        t.muted = p.surface;
        t.muted_foreground = p.text_muted;
        t.popover = p.elevated;
        t.popover_foreground = p.text;
        t.accent = p.hover;
        t.accent_foreground = p.text;
        t.ring = p.accent;
        t.title_bar = p.surface;
        t.title_bar_border = p.border;
        t.status_bar = p.surface;
        t.status_bar_border = p.border;
    });
}


#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: gpui::Hsla) -> f32 {
        let c = gpui::Rgba::from(c);
        let ch = |v: f32| if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
        0.2126 * ch(c.r) + 0.7152 * ch(c.g) + 0.0722 * ch(c.b)
    }

    fn contrast(a: gpui::Hsla, b: gpui::Hsla) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn light_palette_is_readable() {
        let p = Palette::light();
        assert!(contrast(p.text, p.bg) >= 7.0, "text on bg");
        assert!(contrast(p.text_muted, p.bg) >= 4.5, "muted text on bg");
        assert!(contrast(p.text, p.surface) >= 7.0, "text on surface");
    }

    #[test]
    fn hex_parses_short_and_long() {
        assert_eq!(hex("#fff"), rgb(0xffffff));
        assert_eq!(hex("#123456"), rgb(0x123456));
        assert_eq!(hex("bad!"), rgb(0x9ca3af));
    }

    #[test]
    fn mix_is_linear() {
        let m = mix(rgb(0xffffff), 0.5, rgb(0x000000));
        assert!((m.r - 0.5).abs() < 1e-6 && (m.a - 1.).abs() < 1e-6);
    }
}
