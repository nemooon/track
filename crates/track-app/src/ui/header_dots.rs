//! The header's decoration: a grid of small squares tilted by ANGLE, each a random shade of the
//! text color, fading out toward the left. The shades come from a seed, so the pattern holds
//! still between redraws and changes only when the seed does (when data changes, a few times
//! in a row, each fading in from the last).
use gpui::{Bounds, Hsla, PathBuilder, Pixels, Window, point, px};

/// The grid's tilt, in degrees.
pub const ANGLE: f32 = 2.;
/// Distance between dots.
pub const SPACING: f32 = 4.;
/// A dot's side.
pub const SIDE: f32 = 2.5;
/// The darkest a dot gets (alpha of the text color).
pub const STRENGTH: f32 = 0.15;
/// How much fainter the left edge is than the right (0 = even, 1 = gone).
pub const FADE: f32 = 0.8;
/// Shades are drawn in this many steps (one path each).
const STEPS: usize = 8;

/// One dot: its center (relative to the area's top-left) and alpha.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dot {
    pub x: f32,
    pub y: f32,
    pub alpha: f32,
}

/// A number in [0, 1) for grid cell (i, j) under `seed` (splitmix64), the same every time.
fn shade(seed: u64, i: i32, j: i32) -> f32 {
    let mut z = seed ^ ((i as u32 as u64) << 32 | j as u32 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

/// The dots that fall inside a `width` × `height` area for `seed`. Most are faint: the random
/// number is raised to 2.2 before scaling by STRENGTH.
#[cfg(test)]
pub fn dots(seed: u64, width: f32, height: f32) -> Vec<Dot> {
    dots_between(seed, seed, 1., width, height)
}

/// The pattern `t` of the way (0 to 1) from `from`'s shades to `to`'s: the dots stay put and
/// each one's shade moves between the two.
pub fn dots_between(from: u64, to: u64, t: f32, width: f32, height: f32) -> Vec<Dot> {
    let (sin, cos) = ANGLE.to_radians().sin_cos();
    let n = ((width.powi(2) + height.powi(2)).sqrt() / SPACING).ceil() as i32 + 1;
    let mut out = Vec::new();
    for j in -n..=n {
        for i in -n..=n {
            // Grid point (i, j), rotated about the area's left-middle.
            let (gx, gy) = (i as f32 * SPACING, j as f32 * SPACING);
            let x = gx * cos - gy * sin;
            let y = height / 2. + gx * sin + gy * cos;
            if x < 0. || x > width || y < 0. || y > height {
                continue;
            }
            let fade = 1. - FADE * (1. - x / width);
            let level = |seed| shade(seed, i, j).powf(2.2);
            let alpha = ((1. - t) * level(from) + t * level(to)) * STRENGTH * fade;
            if alpha >= 0.005 {
                out.push(Dot { x, y, alpha });
            }
        }
    }
    out
}

/// Paints `dots_between(from, to, t, …)` over `bounds` in `color`, each a square tilted with
/// the grid.
pub fn paint(bounds: Bounds<Pixels>, (from, to, t): (u64, u64, f32), color: Hsla, window: &mut Window) {
    let (x0, y0) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    let (sin, cos) = ANGLE.to_radians().sin_cos();
    let h = SIDE / 2.;
    // Corner offsets of a tilted square.
    let corners = [(-h, -h), (h, -h), (h, h), (-h, h)].map(|(dx, dy)| (dx * cos - dy * sin, dx * sin + dy * cos));
    let mut steps: Vec<Option<PathBuilder>> = (0..STEPS).map(|_| None).collect();
    for d in dots_between(from, to, t, f32::from(bounds.size.width), f32::from(bounds.size.height)) {
        let step = ((d.alpha / STRENGTH) * STEPS as f32).floor().min(STEPS as f32 - 1.) as usize;
        let b = steps[step].get_or_insert_with(PathBuilder::fill);
        let at = |(dx, dy): (f32, f32)| point(px(x0 + d.x + dx), px(y0 + d.y + dy));
        b.move_to(at(corners[0]));
        for c in &corners[1..] {
            b.line_to(at(*c));
        }
        b.close();
    }
    window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
        for (k, b) in steps.into_iter().enumerate() {
            if let Some(path) = b.and_then(|b| b.build().ok()) {
                let alpha = STRENGTH * (k as f32 + 0.5) / STEPS as f32;
                window.paint_path(path, Hsla { a: color.a * alpha, ..color });
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_pattern() {
        assert_eq!(dots(7, 600., 34.), dots(7, 600., 34.));
        assert_ne!(dots(7, 600., 34.), dots(8, 600., 34.));
    }

    #[test]
    fn a_fade_runs_from_one_pattern_to_the_next() {
        assert_eq!(dots_between(1, 2, 0., 600., 34.), dots(1, 600., 34.), "starts on the old");
        assert_eq!(dots_between(1, 2, 1., 600., 34.), dots(2, 600., 34.), "ends on the new");
        let mid = dots_between(1, 2, 0.5, 600., 34.);
        assert!(mid != dots(1, 600., 34.) && mid != dots(2, 600., 34.), "in between halfway");
    }

    #[test]
    fn dots_stay_inside_and_faint() {
        let ds = dots(3, 600., 34.);
        assert!(ds.len() > 100);
        assert!(ds.iter().all(|d| (0. ..=600.).contains(&d.x) && (0. ..=34.).contains(&d.y) && d.alpha <= STRENGTH));
    }

    #[test]
    fn the_left_is_fainter_than_the_right() {
        let ds = dots(5, 1200., 34.);
        let mean = |f: &dyn Fn(&Dot) -> bool| {
            let v: Vec<f32> = ds.iter().filter(|d| f(d)).map(|d| d.alpha).collect();
            v.iter().sum::<f32>() / v.len() as f32
        };
        assert!(mean(&|d| d.x < 300.) < mean(&|d| d.x > 900.) * 0.6);
    }
}
