//! Colours copied from the React client's Tailwind classes / index.css.
use gpui::{Hsla, Rgba, rgb, rgba};

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
pub fn mix(a: Rgba, p: f32, b: Rgba) -> Rgba {
    Rgba { r: a.r * p + b.r * (1. - p), g: a.g * p + b.g * (1. - p), b: a.b * p + b.b * (1. - p), a: a.a * p + b.a * (1. - p) }
}

pub fn with_alpha(c: Rgba, a: f32) -> Hsla {
    Rgba { a, ..c }.into()
}

pub const NO_PROJECT: &str = "#9ca3af";

pub fn header_bg() -> Rgba { rgb(0x2e3a35) }
pub fn header_border() -> Rgba { rgb(0x1d2824) }
pub fn white() -> Rgba { rgb(0xffffff) }
pub fn n50() -> Rgba { rgb(0xfafafa) }
pub fn n100() -> Rgba { rgb(0xf5f5f5) }
pub fn n200() -> Rgba { rgb(0xe5e5e5) }
pub fn n300() -> Rgba { rgb(0xd4d4d4) }
pub fn n400() -> Rgba { rgb(0xa3a3a3) }
pub fn n500() -> Rgba { rgb(0x737373) }
pub fn n600() -> Rgba { rgb(0x525252) }
pub fn n700() -> Rgba { rgb(0x404040) }
pub fn n800() -> Rgba { rgb(0x262626) }
pub fn n900() -> Rgba { rgb(0x171717) }
pub fn amber50() -> Rgba { rgb(0xfffbeb) }
#[allow(dead_code)]
pub fn amber100() -> Rgba { rgb(0xfef3c7) }
#[allow(dead_code)]
pub fn amber500() -> Rgba { rgb(0xf59e0b) }
pub fn amber600() -> Rgba { rgb(0xd97706) }
#[allow(dead_code)]
pub fn amber700() -> Rgba { rgb(0xb45309) }
pub fn red500() -> Rgba { rgb(0xef4444) }
pub fn red50() -> Rgba { rgb(0xfef2f2) }
pub fn red600() -> Rgba { rgb(0xdc2626) }
pub fn red700() -> Rgba { rgb(0xb91c1c) }
pub fn blue500() -> Rgba { rgb(0x3b82f6) }
pub fn black_a(a: f32) -> Rgba { rgba((a * 255.) as u32) }
pub fn white_a(a: f32) -> Rgba { rgba(0xffffff00 | ((a * 255.) as u32)) }

#[cfg(test)]
mod tests {
    use super::*;

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
