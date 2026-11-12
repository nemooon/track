//! 「AI 連携をインストール」: installs the Codex skill and/or the Claude Code command.
//! While the GPUI version is in development they go in under the `track-dev` names.
use crate::ui::theme::*;
use gpui::{Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px};
use std::path::PathBuf;
use track_core::integrations::{IntegrationKind, install_integrations_in};

/// Remove when the GPUI version replaces the Tauri app (the real names are `track`).
pub const DEV_NAMES: bool = true;

pub struct IntegrationsDialog {
    home: PathBuf,
    pub status: Option<Result<Vec<PathBuf>, String>>,
}

impl IntegrationsDialog {
    pub fn new(home: PathBuf) -> Self {
        IntegrationsDialog { home, status: None }
    }

    fn install(&mut self, kind: IntegrationKind, cx: &mut Context<Self>) {
        self.status = Some(install_integrations_in(&self.home, kind, DEV_NAMES).map_err(|e| e.to_string()));
        cx.notify();
    }
}

impl Render for IntegrationsDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let choice = |id: &'static str, label: &'static str, hint: &'static str, kind: IntegrationKind| {
            div()
                .id(id)
                .debug_selector(move || id.into())
                .flex()
                .flex_col()
                .gap(px(2.))
                .rounded_md()
                .border_1()
                .border_color(n200())
                .px_3()
                .py_2()
                .cursor_pointer()
                .hover(|s| s.bg(n50()))
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(n900()).child(label))
                .child(div().text_xs().text_color(n500()).child(hint))
                .on_click(cx.listener(move |this, _, _, cx| this.install(kind, cx)))
        };
        let name = if DEV_NAMES { "track-dev" } else { "track" };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .text_sm()
            .child(div().text_color(n600()).child(format!(
                "AI から Track に工数を登録するためのスキルをインストールします。インストール後、Codex では ${name}、Claude Code では /{name} で呼び出せます。"
            )))
            .when(DEV_NAMES, |d| d.child(div().text_xs().text_color(n500()).child("開発版のため、本来の track スキルとは別の名前で入れ、コピー DB（~/.track-gpui）を使います。")))
            .child(choice("install-codex", "Codex", "~/.codex/skills にスキルを入れる", IntegrationKind::Codex))
            .child(choice("install-claude", "Claude Code", "~/.claude/commands にコマンドを入れる", IntegrationKind::Claude))
            .child(choice("install-both", "両方", "Codex と Claude Code の両方に入れる", IntegrationKind::Both))
            .when_some(self.status.clone(), |d, status| match status {
                Ok(paths) => d.child(
                    div()
                        .debug_selector(|| "install-done".into())
                        .rounded_md()
                        .bg(gpui::rgb(0xecfdf5))
                        .px_3()
                        .py_2()
                        .text_xs()
                        .text_color(gpui::rgb(0x065f46))
                        .child("インストールしました")
                        .children(paths.into_iter().map(|p| div().child(p.display().to_string()))),
                ),
                Err(e) => d.child(div().debug_selector(|| "install-error".into()).rounded_md().bg(red50()).px_3().py_2().text_xs().text_color(red700()).child(e)),
            })
    }
}
