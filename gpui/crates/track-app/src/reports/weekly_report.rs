//! 「週報」 dialog (ReportsPage.tsx generateWeeklyReport). The Markdown is edited as plain text.
use super::custom_output::provider_label;
use crate::data::AiStreamEvent;
use track_core::model::{ReportQuery};
use crate::data::DataClient;
use crate::app::AppState;
use track_core::report::weekly::{build_weekly_report_prompt, period_label, replace_markdown_section};
use crate::ui::theme::*;
use chrono::NaiveDate;
use gpui::{
    AppContext, ClipboardItem, Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Task,
    Window, div, prelude::FluentBuilder, px,
};
use gpui_component::WindowExt as _;
use gpui_component::input::{Textarea, TextareaState};
use gpui_component::{Icon, IconName};
use std::time::Duration;

pub struct WeeklyReport {
    state: Entity<AppState>,
    api: DataClient,
    query: ReportQuery,
    anchor: NaiveDate,
    pub generating: bool,
    pub error: Option<String>,
    pub editor: Entity<TextareaState>,
    pub copied: bool,
    task: Option<Task<()>>,
}

impl WeeklyReport {
    pub fn new(state: Entity<AppState>, query: ReportQuery, anchor: NaiveDate, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let api = state.read(cx).api.clone();
        let editor = cx.new(|cx| TextareaState::new(window, cx).rows(18));
        let mut this = Self { state, api, query, anchor, generating: false, error: None, editor, copied: false, task: None };
        this.generate(window, cx);
        this
    }

    pub fn generate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        self.copied = false;
        self.editor.update(cx, |s, cx| s.set_value("", window, cx));
        let template = self.state.read(cx).data.as_ref().map(|d| d.settings.weekly_report_template.clone()).unwrap_or_default();
        if template.trim().is_empty() {
            self.error = Some("設定の「レポート」で出力テンプレートを保存してください。".into());
            cx.notify();
            return;
        }
        self.generating = true;
        let api = self.api.clone();
        let q = self.query.clone();
        let period = period_label(self.anchor, false);
        cx.notify();
        self.task = Some(cx.spawn_in(window, async move |this, cx| {
            let result: Result<String, String> = async {
                let entries = api.report_entries(&q).await.map_err(|e| e.message)?;
                if entries.entries.is_empty() {
                    return Err("この週には週報に含める工数がありません。".into());
                }
                let (prompt, main_work) = build_weekly_report_prompt(&template, &period, &entries);
                let rx = api.generate_stream(track_core::ai::AiMode::WeeklyReport, prompt);
                let mut text = None;
                while let Ok(ev) = rx.recv().await {
                    match ev {
                        AiStreamEvent::Result { text: t, .. } => text = Some(t),
                        AiStreamEvent::Error { message } => return Err(message),
                        _ => {}
                    }
                }
                let text = text.ok_or("週報を生成できませんでした。")?;
                Ok(replace_markdown_section(&text, "主な作業", &main_work))
            }
            .await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.generating = false;
                match result {
                    Ok(md) => this.editor.update(cx, |s, cx| s.set_value(md, window, cx)),
                    Err(e) => this.error = Some(e),
                }
                cx.notify();
            });
        }));
    }

    pub fn copy(&mut self, cx: &mut Context<Self>) {
        let text = self.editor.read(cx).value().to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.copied = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let _ = this.update(cx, |this, cx| {
                this.copied = false;
                cx.notify();
            });
        })
        .detach();
    }
}

fn button(id: &'static str) -> gpui::Stateful<gpui::Div> {
    div().id(id).debug_selector(move || id.into()).flex().items_center().gap(px(6.)).rounded_md().px_3().py(px(6.)).text_sm().cursor_pointer()
}

impl Render for WeeklyReport {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let provider = self.state.read(cx).data.as_ref().map(|d| d.config.ai_provider.clone()).unwrap_or_default();
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().text_sm().text_color(p.text_muted).child(format!("{} · 表示中の絞り込みを反映", period_label(self.anchor, false))))
            .map(|d| {
                if self.generating {
                    d.child(
                        div()
                            .min_h(px(288.))
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap_3()
                            .text_sm()
                            .text_color(p.text_muted)
                            .child(Icon::new(IconName::LoaderCircle).size_6().text_color(p.success))
                            .child(format!("{}で生成しています…", provider_label(&provider))),
                    )
                } else if let Some(e) = &self.error {
                    d.child(div().debug_selector(|| "weekly-error".into()).rounded_md().border_1().border_color(p.danger_border).bg(p.danger_bg).px_4().py_3().text_sm().text_color(p.danger).child(e.clone()))
                } else {
                    d.child(div().h(px(420.)).child(Textarea::new(&self.editor).h_full()))
                }
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(button("weekly-close").hover(|s| s.bg(p.hover)).child("閉じる").on_click(cx.listener(|_, _, w, cx| w.close_dialog(cx))))
                    .child(
                        button("weekly-regenerate")
                            .border_1()
                            .border_color(p.border)
                            .map(|d| if self.generating { d.opacity(0.5) } else { d.hover(|s| s.bg(p.surface)) })
                            .child("再生成")
                            .on_click(cx.listener(|this, _, w, cx| {
                                if !this.generating {
                                    this.generate(w, cx)
                                }
                            })),
                    )
                    .when(!self.generating && self.error.is_none(), |d| {
                        d.child(
                            button("weekly-copy")
                                .bg(p.text)
                                .text_color(p.bg)
                                .hover(|s| s.bg(p.text))
                                .child(Icon::new(if self.copied { IconName::Check } else { IconName::Copy }).size_4())
                                .child(if self.copied { "コピーしました" } else { "週報をコピー" })
                                .on_click(cx.listener(|this, _, _, cx| this.copy(cx))),
                        )
                    }),
            )
    }
}
