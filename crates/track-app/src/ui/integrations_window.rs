//! 「AI 連携…」: a window of its own with Codex and Claude Code in two areas split by a line. Each
//! shows where it goes, how it stands, and install (update) / uninstall. Everyday (guarded)
//! builds use the `track-dev` names, so the real `track` skill and command are left alone; the
//! packaged app manages the real ones.
use crate::ui::theme::*;
use gpui::{
    AnyWindowHandle, App, AppContext, Context, FontWeight, Global, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled,
    TitlebarOptions, WeakEntity, Window, WindowBounds, WindowOptions, div, prelude::FluentBuilder, px, size,
};
use gpui_component::Root;
use std::path::PathBuf;
use track_core::integrations::{IntegrationKind, IntegrationStatus, install_integrations_in, integration_status, uninstall_integration_in};

/// `track-dev` names in guarded builds, the real `track` in the packaged app.
pub const DEV_NAMES: bool = track_core::guard::GUARDED;

pub struct IntegrationsWindow {
    home: PathBuf,
    /// The last install / uninstall's message per tool (Codex, Claude Code).
    messages: [Option<Result<String, String>>; 2],
}

struct OpenIntegrations {
    window: AnyWindowHandle,
    view: WeakEntity<IntegrationsWindow>,
}

impl Global for OpenIntegrations {}

pub fn integrations_window(cx: &App) -> Option<(AnyWindowHandle, gpui::Entity<IntegrationsWindow>)> {
    let open = cx.try_global::<OpenIntegrations>()?;
    let view = open.view.upgrade()?;
    cx.windows().contains(&open.window).then_some((open.window, view))
}

/// Opens the window for `home`, or brings the open one forward.
pub fn open_integrations(home: PathBuf, cx: &mut App) {
    if let Some((window, view)) = integrations_window(cx) {
        let _ = window.update(cx, |_, window, _| window.activate_window());
        view.update(cx, |v, cx| {
            v.home = home;
            cx.notify();
        });
        return;
    }
    let bounds = gpui::Bounds::centered(None, size(px(520.), px(420.)), cx);
    let title = if DEV_NAMES { "AI 連携（開発用）" } else { "AI 連携" };
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions { title: Some(title.into()), appears_transparent: true, traffic_light_position: Some(traffic_light_position()) }),
            is_minimizable: false,
            ..Default::default()
        },
        |window, cx| {
            let view = cx.new(|_| IntegrationsWindow { home, messages: [None, None] });
            cx.set_global(OpenIntegrations { window: window.window_handle(), view: view.downgrade() });
            cx.new(|cx| Root::new(view, window, cx))
        },
    );
    if let Ok(handle) = opened {
        let _ = handle.update(cx, |_, window, _| window.activate_window());
    }
}

fn slot(kind: IntegrationKind) -> usize {
    if kind == IntegrationKind::Claude { 1 } else { 0 }
}

impl IntegrationsWindow {
    fn install(&mut self, kind: IntegrationKind, cx: &mut Context<Self>) {
        self.messages[slot(kind)] = Some(install_integrations_in(&self.home, kind, DEV_NAMES).map(|_| "インストールしました".to_string()).map_err(|e| e.to_string()));
        cx.notify();
    }

    fn uninstall(&mut self, kind: IntegrationKind, cx: &mut Context<Self>) {
        self.messages[slot(kind)] = Some(uninstall_integration_in(&self.home, kind, DEV_NAMES).map(|_| "アンインストールしました".to_string()).map_err(|e| e.to_string()));
        cx.notify();
    }

    fn area(&self, kind: IntegrationKind, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let key = if kind == IntegrationKind::Claude { "claude" } else { "codex" };
        let name = if DEV_NAMES { "track-dev" } else { "track" };
        let (title, place, call) = match kind {
            IntegrationKind::Claude => ("Claude Code", format!("~/.claude/commands/{name}.md"), format!("/{name}")),
            _ => ("Codex", format!("~/.codex/skills/{name}"), format!("${name}")),
        };
        let status = integration_status(&self.home, kind, DEV_NAMES);
        let (label, colour, state_key) = match &status {
            IntegrationStatus::NotInstalled => ("インストールしていません".to_string(), p.text_muted, "not-installed"),
            IntegrationStatus::Installed => ("インストール済み".to_string(), p.success, "installed"),
            IntegrationStatus::Outdated => ("古い版が入っています".to_string(), p.warning_text, "outdated"),
            IntegrationStatus::Conflict(m) => (m.clone(), p.danger, "conflict"),
        };
        let installable = matches!(status, IntegrationStatus::NotInstalled | IntegrationStatus::Outdated | IntegrationStatus::Installed);
        let removable = matches!(status, IntegrationStatus::Installed | IntegrationStatus::Outdated);
        let install_label = match status {
            IntegrationStatus::Outdated => "更新",
            IntegrationStatus::Installed => "入れ直す",
            _ => "インストール",
        };
        let button = |id: String, label: &'static str, primary: bool| {
            div()
                .id(gpui::SharedString::from(id.clone()))
                .debug_selector(move || id.clone())
                .px_3()
                .py_1()
                .rounded_md()
                .text_size(px(13.))
                .cursor_pointer()
                .map(|d| if primary { d.bg(p.primary).text_color(p.on_color).hover(move |s| s.bg(p.primary_hover)) } else { d.border_1().border_color(p.border).text_color(p.text).hover(move |s| s.bg(p.hover)) })
                .child(label)
        };
        let message = self.messages[slot(kind)].clone();
        div()
            .debug_selector(move || format!("integration-{key}"))
            .flex()
            .flex_col()
            .gap_1()
            .py_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().flex_1().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(p.text).child(title))
                    .when(removable, |d| d.child(button(format!("integration-{key}-uninstall"), "アンインストール", false).on_click(cx.listener(move |this, _, _, cx| this.uninstall(kind, cx)))))
                    .when(installable, |d| d.child(button(format!("integration-{key}-install"), install_label, true).on_click(cx.listener(move |this, _, _, cx| this.install(kind, cx))))),
            )
            .child(div().debug_selector(move || format!("integration-{key}-{state_key}")).text_size(px(12.)).text_color(colour).child(label))
            .child(div().text_size(px(12.)).text_color(p.text_muted).child(format!("{place}　呼び出し: {call}")))
            .when_some(message, |d, m| match m {
                Ok(text) => d.child(div().text_size(px(12.)).text_color(p.success).child(text)),
                Err(e) => d.child(div().debug_selector(move || format!("integration-{key}-error")).text_size(px(12.)).text_color(p.danger).child(e)),
            })
    }
}

impl Render for IntegrationsWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        div()
            .size_full()
            .flex()
            .flex_col()
            .px_5()
            .pt(px(44.))
            .pb_4()
            .bg(p.bg)
            .text_color(p.text)
            .child(div().text_size(px(13.)).text_color(p.text_muted).child("AI から Track に工数を記録するためのスキルを、AI ツールごとに入れたり外したりします。"))
            .when(DEV_NAMES, |d| d.child(div().mt_1().text_size(px(12.)).text_color(p.text_muted).child("開発版のため、本来の track とは別の名前（track-dev）で入れ、コピー DB（~/.track-gpui）を使います。")))
            .child(self.area(IntegrationKind::Codex, p, cx))
            .child(div().debug_selector(|| "integration-divider".into()).h(px(1.)).bg(p.border))
            .child(self.area(IntegrationKind::Claude, p, cx))
    }
}
