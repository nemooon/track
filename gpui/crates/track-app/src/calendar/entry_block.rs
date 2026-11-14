//! EntryBlock.tsx: one time entry inside a day column.
use track_core::model::TimeEntry;
use crate::core::geometry::minutes_to_y;
use crate::core::time::format_duration;
use crate::persist::DurationStyle;
use crate::ui::theme::*;
use gpui::{
    BoxShadow, Div, FontWeight, InteractiveElement, ParentElement, Stateful, Styled, div, point,
    prelude::FluentBuilder, px,
};

pub struct BlockGeometry {
    pub start_min: i64,
    pub end_min: i64,
    pub lane: usize,
    pub lane_count: usize,
    /// Lanes covered from `lane` rightwards (free lanes beside it).
    pub span: usize,
}

/// The title row's height: a title kept in view never goes lower than this above the block's end.
pub const TITLE_ROW: f32 = 18.;

/// Below this width the duration goes under the title, so the title keeps the row.
const BESIDE_MIN_WIDTH: f32 = 120.;

pub fn break_label(break_min: i64, style: DurationStyle) -> String {
    if break_min <= 0 {
        String::new()
    } else {
        format!("（休憩 {} 込み）", format_duration(break_min, style))
    }
}

/// How far down a block's content moves so its title stays at the top of the view while the
/// block's top is scrolled out (`visible_top` and `block_top` in the same column coordinates).
pub fn sticky_title_offset(block_top: f32, block_height: f32, visible_top: f32) -> f32 {
    (visible_top - block_top).clamp(0., (block_height - TITLE_ROW).max(0.))
}

/// Whether the duration fits beside the title in a block this wide (else it goes underneath).
pub fn duration_beside_title(width: f32) -> bool {
    width >= BESIDE_MIN_WIDTH
}

/// Outer absolutely-positioned lane slot; the block itself sits at left 2px / right 4px
/// inside it, which equals `left: lane*100%/n + 2px; width: 100%/n - 6px`.
pub fn lane_slot(g: &BlockGeometry, hour_px: f32) -> Div {
    let top = minutes_to_y(g.start_min, hour_px) + 2.;
    let height = (minutes_to_y(g.end_min - g.start_min, hour_px) - 5.).max(14.);
    div()
        .absolute()
        .top(px(top))
        .h(px(height))
        .left(gpui::relative(g.lane as f32 / g.lane_count as f32))
        .w(gpui::relative(g.span.max(1) as f32 / g.lane_count as f32))
}

/// How a block is drawn: the duration style, how far its title is pushed down to stay in view,
/// and whether the duration fits beside the title.
pub struct BlockLook {
    pub style: DurationStyle,
    pub title_top: f32,
    pub beside: bool,
}

pub fn render_block(p: Palette, entry: &TimeEntry, selected: bool, ghost: bool, hovered_id: &str, look: BlockLook) -> Stateful<Div> {
    let accent = hex(entry.project.as_ref().map(|p| p.color.as_str()).unwrap_or(NO_PROJECT));
    let hovered = hovered_id == entry.id;
    // A pale wash of the project colour with a slightly stronger rim (Zed-like, no heavy shadow).
    let (border, bg) = if selected {
        (mix(accent, 0.6, p.bg), mix(accent, 0.18, p.bg))
    } else if hovered {
        (mix(accent, 0.45, p.bg), mix(accent, 0.15, p.bg))
    } else {
        (mix(accent, 0.3, p.bg), mix(accent, 0.12, p.bg))
    };
    let text = p.text;
    let gross = (entry.end - entry.start).num_minutes();
    let dur = format!("{}{}", format_duration((gross - entry.break_minutes).max(0), look.style), break_label(entry.break_minutes, look.style));
    let project_name = entry.project.as_ref().map(|p| p.name.clone()).unwrap_or_else(|| "プロジェクトなし".into());
    let title = entry.title.clone().filter(|t| !t.is_empty());
    let mut shadows = vec![];
    if selected {
        shadows.push(BoxShadow { color: with_alpha(accent, 0.22), offset: point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(2.), inset: false });
    }
    let id_owned = entry.id.clone();
    div()
        .id(gpui::SharedString::from(format!("entry-{id_owned}")))
        .debug_selector(move || format!("entry-{id_owned}"))
        .absolute()
        .top_0()
        .bottom_0()
        .left(px(2.))
        .right(px(4.))
        .overflow_hidden()
        .rounded_md()
        .border_1()
        .border_color(border)
        .bg(bg)
        .text_color(text)
        .px_2()
        .py_1()
        .text_xs()
        .shadow(shadows)
        .when(ghost, |d| d.opacity(0.6))
        .cursor(if ghost { gpui::CursorStyle::ClosedHand } else { gpui::CursorStyle::OpenHand })
        .child({
            let id = entry.id.clone();
            let sel = move |part: &'static str| {
                let id = id.clone();
                move || format!("entry-{id}-{part}")
            };
            // Title with its duration at the right end, then project and tags, left-aligned. Short
            // entries show as many lines as fit (a 15-minute one shows the title line).
            // A long block scrolled past its top keeps its title in view (pushed down by title_top);
            // a narrow one puts the duration under the title so the title keeps the width.
            let duration = div().debug_selector(sel("duration")).flex_shrink_0().text_size(px(11.)).line_height(px(15.)).font_family(MONO).text_color(p.text_muted).child(dur);
            let title_el = div().debug_selector(sel("title")).flex_1().min_w_0().truncate().font_weight(FontWeight::SEMIBOLD).line_height(px(15.)).child(title.clone().unwrap_or(project_name.clone()));
            div()
                .flex()
                .flex_col()
                .h_full()
                .overflow_hidden()
                .pt(px(look.title_top))
                .map(|d| {
                    if look.beside {
                        d.child(div().flex().items_baseline().gap(px(6.)).child(title_el).child(duration))
                    } else {
                        d.child(title_el).child(duration)
                    }
                })
                .when(title.is_some(), |d| {
                    d.child(div().debug_selector(sel("project")).truncate().text_size(px(11.)).line_height(px(14.)).text_color(p.text_muted).child(project_name.clone()))
                })
                .when(!entry.tags.is_empty(), |d| {
                    d.child(
                        div()
                            .mt(px(2.))
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_x(px(6.))
                            .gap_y(px(2.))
                            .text_size(px(10.))
                            .line_height(px(13.))
                            .text_color(p.text_muted)
                            .children(entry.tags.iter().map(|t| {
                                div()
                                    .flex()
                                    .min_w_0()
                                    .items_center()
                                    .gap_1()
                                    .child(div().size(px(6.)).flex_shrink_0().rounded_full().bg(hex(&t.tag.color)))
                                    .child(div().truncate().child(t.tag.name.clone()))
                            })),
                    )
                })
        })
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_follows_the_top_of_the_view() {
        // A block from y=100 to y=400; the title row is TITLE_ROW tall.
        assert_eq!(sticky_title_offset(100., 300., 50.), 0., "block top visible");
        assert_eq!(sticky_title_offset(100., 300., 180.), 80., "scrolled into the block");
        assert_eq!(sticky_title_offset(100., 300., 390.), 300. - TITLE_ROW, "never below the block");
    }

    #[test]
    fn narrow_blocks_put_the_duration_under_the_title() {
        assert!(duration_beside_title(160.));
        assert!(!duration_beside_title(90.));
    }

    #[test]
    fn break_label_formats() {
        let hm = DurationStyle::HoursMinutes;
        assert_eq!(break_label(0, hm), "");
        assert_eq!(break_label(45, hm), "（休憩 45m 込み）");
        assert_eq!(break_label(90, hm), "（休憩 1h 30m 込み）");
        assert_eq!(break_label(90, DurationStyle::Decimal), "（休憩 1.5h 込み）");
    }
}
