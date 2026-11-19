//! 「アップデートを確認…」: a window of its own that checks GitHub for a newer release, shows its
//! release notes, and updates through Homebrew (as the Tauri version did), then offers a restart.
use crate::ui::theme::*;
use gpui::{
    AnyWindowHandle, App, AppContext, Context, Global, InteractiveElement, IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    TitlebarOptions, WeakEntity, Window, WindowBounds, WindowOptions, div, prelude::FluentBuilder, px, size,
};
use gpui_component::Root;
use gpui_component::text::TextView;
use std::sync::Arc;
use track_core::update::{Release, is_newer};

/// Where the newest release comes from and how Track is updated (GitHub and Homebrew; tests swap
/// in stand-ins). Both run off the main thread.
/// Reads the newest release.
pub type Fetch = Arc<dyn Fn() -> Result<Release, String> + Send + Sync>;
/// Updates Track to a release.
pub type Install = Arc<dyn Fn(&Release) -> Result<(), String> + Send + Sync>;

#[derive(Clone)]
pub struct UpdateSource {
    pub fetch: Fetch,
    pub install: Install,
}

impl Global for UpdateSource {}

impl UpdateSource {
    /// GitHub's latest release and `brew upgrade --cask`. A development build never updates
    /// /Applications (the dev guard).
    fn real() -> Self {
        UpdateSource {
            fetch: Arc::new(track_core::update::fetch_latest),
            install: Arc::new(|release: &Release| {
                if track_core::guard::GUARDED {
                    return Err("開発版ではアップデートしません。".into());
                }
                let brew = track_core::update::homebrew_executable().ok_or("Homebrewが見つかりません。ターミナルからアップデートしてください。")?;
                let app = track_core::update::current_app_bundle().ok_or("Track.appのインストール先を確認できません。")?;
                track_core::update::run_homebrew_update(&brew, std::path::Path::new(track_core::update::XATTR), &app, &release.version)
            }),
        }
    }
}

/// Swaps in a stand-in for GitHub and Homebrew (tests).
#[cfg(test)]
pub fn set_source(source: UpdateSource, cx: &mut App) {
    cx.set_global(source);
}

/// A sample newer release whose update pretends to succeed (after a moment), to look at the
/// window without a real release. Used by development builds started with
/// `TRACK_GPUI_UPDATE_PREVIEW=<version>`.
pub fn preview_source(version: &str) -> UpdateSource {
    let version = version.to_string();
    UpdateSource {
        fetch: Arc::new(move || {
            Ok(Release {
                version: version.clone(),
                notes: "## 新しくなったこと\n\n- これは確認用の見本のリリースノートです\n- **太字**や`コード`、[リンク](https://github.com/nemooon/track)も表示できます\n\n## 直したこと\n\n- 実際のリリースでは、GitHub に書いた内容がここに出ます".into(),
                url: "https://github.com/nemooon/track/releases".into(),
            })
        }),
        install: Arc::new(|_| {
            std::thread::sleep(std::time::Duration::from_secs(2));
            Ok(())
        }),
    }
}

fn source(cx: &App) -> UpdateSource {
    if let Some(s) = cx.try_global::<UpdateSource>() {
        return s.clone();
    }
    // Development builds only: the preview never reaches a packaged app.
    match std::env::var("TRACK_GPUI_UPDATE_PREVIEW") {
        Ok(v) if track_core::guard::GUARDED && !cfg!(test) && !v.is_empty() => preview_source(&v),
        _ => UpdateSource::real(),
    }
}

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

enum Stage {
    Checking,
    Current,
    Available(Release),
    Updating(Release),
    Updated(Release),
    /// The reason, and the release when the check itself worked (so the update can be retried).
    Failed(String, Option<Release>),
}

pub struct UpdateWindow {
    stage: Stage,
}

struct OpenUpdate {
    window: AnyWindowHandle,
    view: WeakEntity<UpdateWindow>,
}

impl Global for OpenUpdate {}

pub fn update_window(cx: &App) -> Option<(AnyWindowHandle, gpui::Entity<UpdateWindow>)> {
    let open = cx.try_global::<OpenUpdate>()?;
    let view = open.view.upgrade()?;
    cx.windows().contains(&open.window).then_some((open.window, view))
}

/// Opens the update window (checking again), or brings the open one forward.
pub fn open_update(cx: &mut App) {
    if let Some((window, view)) = update_window(cx) {
        let _ = window.update(cx, |_, window, _| window.activate_window());
        view.update(cx, |v, cx| v.check(cx));
        return;
    }
    let bounds = gpui::Bounds::centered(None, size(px(480.), px(420.)), cx);
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions { title: Some("アップデート".into()), appears_transparent: true, traffic_light_position: Some(traffic_light_position()) }),
            is_minimizable: false,
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|cx| {
                let mut v = UpdateWindow { stage: Stage::Checking };
                v.check(cx);
                v
            });
            cx.set_global(OpenUpdate { window: window.window_handle(), view: view.downgrade() });
            cx.new(|cx| Root::new(view, window, cx))
        },
    );
    if let Ok(handle) = opened {
        let _ = handle.update(cx, |_, window, _| window.activate_window());
    }
}

impl UpdateWindow {
    fn check(&mut self, cx: &mut Context<Self>) {
        self.stage = Stage::Checking;
        let fetch = source(cx).fetch;
        let task = cx.background_executor().spawn(async move { fetch() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.stage = match result {
                    Ok(r) if is_newer(&r.version, CURRENT_VERSION) => Stage::Available(r),
                    Ok(_) => Stage::Current,
                    Err(e) => Stage::Failed(e, None),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn install(&mut self, release: Release, cx: &mut Context<Self>) {
        self.stage = Stage::Updating(release.clone());
        let install = source(cx).install;
        let r = release.clone();
        let task = cx.background_executor().spawn(async move { install(&r) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.stage = match result {
                    Ok(()) => Stage::Updated(release),
                    Err(e) => Stage::Failed(e, Some(release)),
                };
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

fn button(id: &'static str, label: &'static str, primary: bool, p: Palette) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .px_3()
        .py_1()
        .rounded_md()
        .text_size(px(13.))
        .cursor_pointer()
        .map(|d| if primary { d.bg(p.primary).text_color(p.on_color).hover(move |s| s.bg(p.primary_hover)) } else { d.border_1().border_color(p.border).text_color(p.text).hover(move |s| s.bg(p.hover)) })
        .child(label)
}

/// The same button, greyed out and not pressable (while updating).
fn disabled_button(id: &'static str, label: &'static str, primary: bool, p: Palette) -> gpui::Div {
    div()
        .debug_selector(move || id.into())
        .px_3()
        .py_1()
        .rounded_md()
        .text_size(px(13.))
        .opacity(0.5)
        .map(|d| if primary { d.bg(p.primary).text_color(p.on_color) } else { d.border_1().border_color(p.border).text_color(p.text) })
        .child(label)
}

impl Render for UpdateWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let heading = |t: String| div().text_size(px(15.)).font_weight(gpui::FontWeight::SEMIBOLD).text_color(p.text).child(t);
        let muted = |t: String| div().text_size(px(12.)).text_color(p.text_muted).child(t);
        let notes = |r: &Release| {
            div()
                .id("update-notes-scroll")
                .debug_selector(|| "update-notes".into())
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(p.border)
                .bg(p.surface)
                .text_size(px(13.))
                .child(if r.notes.is_empty() { div().text_color(p.text_muted).child("リリースノートはありません").into_any_element() } else { TextView::markdown(SharedString::from(format!("notes-{}", r.version)), r.notes.clone()).into_any_element() })
        };
        let close = button("update-close", "閉じる", false, p).on_click(|_, window, _| window.remove_window());
        let body = div().size_full().flex().flex_col().gap_3().px_5().pt(px(44.)).pb_4().bg(p.bg).text_color(p.text);
        let _ = window;
        match &self.stage {
            Stage::Checking => body.child(heading("アップデートを確認しています…".into())),
            Stage::Current => body
                .child(div().debug_selector(|| "update-current".into()).child(heading(format!("Track {CURRENT_VERSION} は最新版です"))))
                .child(div().flex_1())
                .child(div().flex().justify_end().child(close)),
            Stage::Available(r) => {
                let r2 = r.clone();
                body.child(heading(format!("新しいバージョン {} があります", r.version)))
                    .child(muted(format!("今のバージョン: {CURRENT_VERSION}")))
                    .child(notes(r))
                    .child(
                        div().flex().justify_end().gap_2().child(button("update-later", "あとで", false, p).on_click(|_, window, _| window.remove_window())).child(
                            button("update-install", "アップデート", true, p).on_click(cx.listener(move |this, _, _, cx| this.install(r2.clone(), cx))),
                        ),
                    )
            }
            // The actions stay where they were, greyed out, so nothing jumps.
            Stage::Updating(r) => body
                .child(heading(format!("バージョン {} にアップデートしています…", r.version)))
                .child(muted("Homebrew からダウンロードしている間も、Track はそのまま使えます。".into()))
                .child(notes(r))
                .child(div().flex().justify_end().gap_2().child(disabled_button("update-later", "あとで", false, p)).child(disabled_button("update-installing", "アップデート中…", true, p))),
            Stage::Updated(r) => body
                .child(div().debug_selector(|| "update-done".into()).child(heading(format!("バージョン {} にアップデートしました", r.version))))
                .child(muted("再起動すると新しいバージョンになります。".into()))
                .child(div().flex_1())
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(button("update-later", "あとで", false, p).on_click(|_, window, _| window.remove_window()))
                        .child(button("update-restart", "再起動", true, p).on_click(|_, _, cx| cx.restart())),
                ),
            Stage::Failed(reason, release) => {
                let release = release.clone();
                body.child(heading(if release.is_some() { "アップデートできませんでした".into() } else { "アップデートを確認できませんでした".into() }))
                    .child(div().debug_selector(|| "update-error".into()).text_size(px(12.)).text_color(p.danger).child(reason.clone()))
                    .when_some(release.as_ref(), |d, r| d.child(notes(r)))
                    .when(release.is_none(), |d| d.child(div().flex_1()))
                    .child(div().flex().justify_end().gap_2().child(close).map(|d| match release {
                        Some(r) => d.child(button("update-install", "もう一度アップデート", true, p).on_click(cx.listener(move |this, _, _, cx| this.install(r.clone(), cx)))),
                        None => d.child(button("update-retry", "もう一度確認", true, p).on_click(cx.listener(|this, _, _, cx| this.check(cx)))),
                    }))
            }
        }
    }
}
