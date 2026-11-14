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

/// Zed's title bar (also gpui-component's `TITLE_BAR_HEIGHT`).
pub const TITLE_BAR_HEIGHT: gpui::Pixels = gpui::px(34.);
/// Where macOS draws the traffic lights so they sit centred in that bar, as in Zed.
pub fn traffic_light_position() -> gpui::Point<gpui::Pixels> {
    gpui::point(gpui::px(9.), gpui::px(9.))
}

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
    pub danger_border: Hsla,
    pub warning_text: Hsla,
    pub warning_border: Hsla,
    /// Success / ready (AI available, latest backup, weekly report).
    pub success: Hsla,
    pub success_bg: Hsla,
    pub success_border: Hsla,
    pub success_hover: Hsla,
    /// Informational panels and the output button.
    pub info: Hsla,
    pub info_bg: Hsla,
    pub info_border: Hsla,
    pub info_hover: Hsla,
    /// AI-generated columns and formats.
    pub ai: Hsla,
    pub ai_bg: Hsla,
    /// Dark primary buttons and their hover.
    pub primary: Hsla,
    pub primary_hover: Hsla,
    /// Text on a saturated fill (primary buttons, tag and holiday chips).
    pub on_color: Hsla,
    /// Attention dot (pending AI change).
    pub warning: Hsla,
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
            danger_border: c(0xfecaca),
            warning_text: c(0x92400e),
            warning_border: c(0xfde68a),
            success: c(0x047857),
            success_bg: c(0xecfdf5),
            success_border: c(0xa7f3d0),
            success_hover: c(0xd1fae5),
            info: c(0x1d4ed8),
            info_bg: c(0xeff6ff),
            info_border: c(0xbfdbfe),
            info_hover: c(0xdbeafe),
            ai: c(0x6d28d9),
            ai_bg: c(0xede9fe),
            primary: c(0x1f2328),
            primary_hover: c(0x3a3f45),
            on_color: c(0xffffff),
            warning: c(0xd97706),
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
    fn primary_button_has_a_hover_shade_and_readable_text() {
        let p = Palette::light();
        assert_ne!(p.primary, p.primary_hover);
        assert!(contrast(p.on_color, p.primary) >= 7.0);
    }

    /// Every colour on screen comes from `Palette`, so a dark palette only needs new values.
    #[test]
    fn no_raw_hex_colours_outside_the_palette() {
        fn walk(dir: &std::path::Path, out: &mut Vec<String>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let path = e.path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|x| x == "rs") {
                    let name = path.to_string_lossy().to_string();
                    if name.ends_with("ui/theme.rs") || name.ends_with("_tests.rs") {
                        continue;
                    }
                    for (i, line) in std::fs::read_to_string(&path).unwrap().lines().enumerate() {
                        if line.contains("rgb(0x") || line.contains("rgba(0x") {
                            out.push(format!("{name}:{}", i + 1));
                        }
                    }
                }
            }
        }
        let mut found = Vec::new();
        walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut found);
        assert!(found.is_empty(), "raw colours outside Palette:\n{}", found.join("\n"));
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
