//! The header's 「…」 menu, as in the Tauri version: keyboard shortcuts, settings, the AI
//! integration and about. It opens from the button (a popover), like every menu in the app.
use crate::app::TrackApp;
use crate::settings::popover::Trigger;
use crate::ui::theme::*;
use gpui::{AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px};
use gpui_component::popover::Popover;
use gpui_component::WindowExt as _;

/// The shortcuts listed in 「キーボードショートカット」, grouped as in the Tauri version.
pub const SHORTCUTS: [(&str, &[(&str, &str)]); 3] = [
    ("画面", &[("カレンダーを開く", "⌘1"), ("レポートを開く", "⌘2"), ("設定を開く", "⌘,")]),
    ("日付", &[("前の期間へ移動", "⌘["), ("次の期間へ移動", "⌘]"), ("今日へ移動", "⌘T")]),
    ("カレンダー", &[("表示を拡大", "⌘+"), ("表示を縮小", "⌘−")]),
];

pub fn render(app: &TrackApp, cx: &mut Context<TrackApp>) -> AnyElement {
    let p = palette(cx);
    // Hover and an open menu darken the dots instead of lighting a background.
    let open = app.app_menu_open;
    let size = crate::app::HEADER_ICON;
    let button = div()
        .id("app-menu")
        .debug_selector(|| "app-menu".into())
        .flex_shrink_0()
        .p_1()
        .cursor_pointer()
        .text_color(if open { p.text } else { p.text_muted })
        .hover(move |s| s.text_color(p.text))
        .child(div().debug_selector(|| "app-menu-icon".into()).size(size).child(crate::ui::icons::ellipsis_vertical().size(size)));
    let this = cx.entity().downgrade();
    let mut pop = Popover::new("app-menu-pop")
        .anchor(gpui::Anchor::TopRight)
        .open(app.app_menu_open)
        .on_open_change(move |open: &bool, _, cx| {
            let open = *open;
            let _ = this.update(cx, |this, cx| {
                this.app_menu_open = open;
                cx.notify();
            });
        })
        .trigger(Trigger::new(button));
    if app.app_menu_open {
        let integrations = if track_core::guard::GUARDED { "AI 連携をインストール…（開発用）" } else { "AI 連携をインストール…" };
        pop = pop.w(px(220.)).p_1().child(
            div()
                .flex()
                .flex_col()
                .text_size(px(13.))
                .child(item("app-menu-shortcuts", "キーボードショートカット", None, p, cx, open_shortcuts))
                .child(div().my(px(3.)).mx_1().h(px(1.)).bg(p.border_subtle))
                .child(item("app-menu-settings", "設定…", Some("⌘,"), p, cx, |this, _, cx| crate::settings::open_settings(this.state.clone(), cx)))
                .child(item("app-menu-integrations", integrations, None, p, cx, |this, window, cx| this.open_integrations(window, cx)))
                .child(div().my(px(3.)).mx_1().h(px(1.)).bg(p.border_subtle))
                .child(item("app-menu-update", "アップデートを確認…", None, p, cx, |_, _, cx| crate::ui::update::open_update(cx)))
                .child(item("app-menu-about", "Track について", None, p, cx, open_about)),
        );
    }
    // The header moves the window on a press: keep presses on the menu (after the popover saw them).
    div().ml_auto().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(pop).into_any_element()
}

/// A menu row: it closes the menu, then does `run`.
fn item(
    id: &'static str,
    label: &'static str,
    shortcut: Option<&'static str>,
    p: Palette,
    cx: &mut Context<TrackApp>,
    run: impl Fn(&mut TrackApp, &mut Window, &mut Context<TrackApp>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .rounded(RADIUS_SM)
        .cursor_pointer()
        .text_color(p.text)
        .hover(move |s| s.bg(p.hover))
        .child(div().flex_1().child(label))
        .when_some(shortcut, |d, s| d.child(div().text_color(p.text_muted).font_family(MONO).child(s)))
        .on_click(cx.listener(move |this, _, window, cx| {
            this.app_menu_open = false;
            run(this, window, cx);
            cx.notify();
        }))
}

fn open_shortcuts(_: &mut TrackApp, window: &mut Window, cx: &mut Context<TrackApp>) {
    let p = palette(cx);
    window.open_dialog(cx, move |dialog, _, _| {
        dialog.title("キーボードショートカット").w(px(400.)).child(
            div().debug_selector(|| "shortcuts-dialog".into()).flex().flex_col().gap_4().children(SHORTCUTS.iter().map(|(group, items)| {
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_xs().font_weight(gpui::FontWeight::SEMIBOLD).text_color(p.text_muted).child(*group))
                    .children(items.iter().map(|(label, keys)| {
                        div().flex().justify_between().text_size(px(13.)).child(div().text_color(p.text).child(*label)).child(div().font_family(MONO).text_color(p.text_muted).child(*keys))
                    }))
            })),
        )
    });
}

fn open_about(_: &mut TrackApp, _: &mut Window, cx: &mut Context<TrackApp>) {
    crate::ui::about::open_about(cx);
}
