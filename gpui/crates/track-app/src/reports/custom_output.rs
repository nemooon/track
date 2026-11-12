//! 「カスタム出力」 dialog (ReportsPage.tsx createCustomOutput / copyAggregation).
use track_core::model::{GroupBy, ReportCopyFormat, ReportEntriesResponse, ReportQuery, ReportRange, ReportRow};
use crate::data::DataClient;
use crate::app::AppState;
use crate::api::ndjson::AiStreamEvent;
use track_core::report::aggregation::{AiAggregationRow, aggregate_ai_result, build_report_aggregation_prompt};
use track_core::report::cache::{CacheKeyInput, CachedOutput, load_output, output_cache_key, save_output};
use track_core::report::copy::{aggregation_column_value, build_aggregation_copy_text, report_copy_column_header, report_copy_format_uses_ai};
use track_core::report::view::{child_query, display_label};
use track_core::report::weekly::period_label;
use crate::core::time::format_duration_report;
use crate::ui::{date_nav::below, theme::*};
use chrono::{Local, NaiveDate, TimeZone};
use gpui::{
    ClipboardItem, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Task,
    Window, div, prelude::FluentBuilder, px,
};
use gpui_component::{Icon, IconName};
use std::time::{Duration, Instant};

pub fn provider_label(p: &str) -> &'static str {
    match p {
        "codex" => "Codex",
        "custom-command" => "カスタムコマンド",
        _ => "Apple Intelligence",
    }
}

pub fn elapsed_label(secs: u64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

pub struct OutputContext {
    pub query: ReportQuery,
    pub anchor: NaiveDate,
    pub rows: Vec<ReportRow>,
    pub total: i64,
}

pub struct CustomOutput {
    state: Entity<AppState>,
    api: DataClient,
    ctx: OutputContext,
    formats: Vec<ReportCopyFormat>,
    pub selected: Option<String>,
    picking_format: bool,
    entries: Option<ReportEntriesResponse>,
    pub generating: bool,
    started: Option<Instant>,
    pub progress: Option<(String, String)>,
    pub error: Option<String>,
    pub result: Vec<AiAggregationRow>,
    result_total: i64,
    pub restored_at: Option<i64>,
    pub copied: bool,
    task: Option<Task<()>>,
    ticker: Option<Task<()>>,
}

impl CustomOutput {
    pub fn new(state: Entity<AppState>, ctx: OutputContext, cx: &mut Context<Self>) -> Self {
        let (api, formats) = {
            let s = state.read(cx);
            let formats: Vec<ReportCopyFormat> = s.data.as_ref().map(|d| d.settings.report_copy_formats.iter().filter(|f| f.target == "ai-aggregation").cloned().collect()).unwrap_or_default();
            (s.api.clone(), formats)
        };
        let selected = formats.first().map(|f: &ReportCopyFormat| f.id.clone());
        let mut this = Self {
            state,
            api,
            ctx,
            formats,
            selected,
            picking_format: false,
            entries: None,
            generating: false,
            started: None,
            progress: None,
            error: None,
            result: vec![],
            result_total: 0,
            restored_at: None,
            copied: false,
            task: None,
            ticker: None,
        };
        this.fetch_entries(cx);
        this
    }

    fn format(&self) -> Option<&ReportCopyFormat> {
        self.selected.as_ref().and_then(|id| self.formats.iter().find(|f| &f.id == id))
    }

    fn cache_key(&self, entries: &ReportEntriesResponse) -> Option<String> {
        let f = self.format()?.clone();
        let q = &self.ctx.query;
        Some(output_cache_key(&CacheKeyInput {
            range: q.range,
            anchor: self.ctx.anchor.format("%Y-%m-%d").to_string(),
            group_by: q.group_by,
            client_ids: q.client_ids.clone(),
            project_ids: q.project_ids.clone(),
            tag_ids: q.tag_ids.clone(),
            report_rows: self.ctx.rows.clone(),
            entries: entries.entries.clone(),
            total_minutes: self.ctx.total,
            copy_format: f,
        }))
    }

    /// Fetch the period's entries (staleTime 0) and restore a cached result for the selected format.
    fn fetch_entries(&mut self, cx: &mut Context<Self>) {
        let fut = self.api.report_entries(&self.ctx.query);
        self.task = Some(cx.spawn(async move |this, cx| {
            if let Ok(e) = fut.await {
                let _ = this.update(cx, |this, cx| {
                    this.entries = Some(e);
                    this.restore(cx);
                });
            }
        }));
    }

    fn restore(&mut self, cx: &mut Context<Self>) {
        self.result.clear();
        self.restored_at = None;
        let Some(key) = self.entries.as_ref().and_then(|e| self.cache_key(e)) else { return };
        let now = chrono::Utc::now().timestamp_millis();
        if let Some(c) = load_output(&self.state.read(cx).persist.output_cache, &key, now).cloned() {
            self.result = c.rows;
            self.result_total = c.total_minutes;
            self.restored_at = Some(c.created_at);
            self.error = None;
        }
        cx.notify();
    }

    pub fn select_format(&mut self, id: String, cx: &mut Context<Self>) {
        self.selected = Some(id);
        self.picking_format = false;
        self.error = None;
        self.progress = None;
        self.restore(cx);
    }

    pub fn create(&mut self, cx: &mut Context<Self>) {
        let Some(format) = self.format().cloned() else {
            self.error = Some("設定の「レポート」で出力フォーマットを登録してください。".into());
            cx.notify();
            return;
        };
        self.generating = true;
        self.result.clear();
        self.error = None;
        self.copied = false;
        self.progress = None;
        self.restored_at = None;
        self.started = Some(Instant::now());
        if self.ctx.rows.is_empty() {
            self.generating = false;
            self.error = Some("この期間には出力する工数がありません。".into());
            cx.notify();
            return;
        }
        self.result_total = self.ctx.total;
        let uses_ai = report_copy_format_uses_ai(&format);
        if uses_ai {
            self.ticker = Some(cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }));
        }
        let api = self.api.clone();
        let q = self.ctx.query.clone();
        let rows = self.ctx.rows.clone();
        let group_by = q.group_by;
        cx.notify();
        self.task = Some(cx.spawn(async move |this, cx| {
            let result: Result<(Vec<AiAggregationRow>, ReportEntriesResponse), String> = async {
                let entries = api.report_entries(&q).await.map_err(|e| e.message)?;
                if uses_ai {
                    let prompt = build_report_aggregation_prompt(&entries, &format);
                    let rx = api.generate_stream("report-aggregation", prompt);
                    let mut text = None;
                    while let Ok(ev) = rx.recv().await {
                        match ev {
                            AiStreamEvent::Progress { kind, message } => {
                                let _ = this.update(cx, |this, cx| {
                                    this.progress = Some((kind, message));
                                    cx.notify();
                                });
                            }
                            AiStreamEvent::Result { text: t, .. } => text = Some(t),
                            AiStreamEvent::Error { message } => return Err(message),
                            AiStreamEvent::Heartbeat => {}
                        }
                    }
                    let text = text.ok_or("AIの応答が途中で終了しました。")?;
                    Ok((aggregate_ai_result(&text, &entries)?, entries))
                } else {
                    let mut out = Vec::new();
                    for r in &rows {
                        let child = api.report_entries(&child_query(&q, group_by, &r.key)).await.map_err(|e| e.message)?;
                        out.push(AiAggregationRow {
                            label: display_label(group_by, &r.label),
                            summary: String::new(),
                            minutes: r.total_minutes,
                            entry_count: child.entries.len() as i64,
                            generated_values: Default::default(),
                        });
                    }
                    Ok((out, entries))
                }
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.generating = false;
                this.ticker = None;
                match result {
                    Ok((rows, entries)) => {
                        if let Some(key) = this.cache_key(&entries) {
                            let now = chrono::Utc::now().timestamp_millis();
                            let entry = CachedOutput { key, created_at: now, total_minutes: this.ctx.total, rows: rows.clone() };
                            this.state.update(cx, |s, _| s.update_persist(|p| save_output(&mut p.output_cache, entry, now)));
                        }
                        this.entries = Some(entries);
                        this.result = rows;
                    }
                    Err(e) => this.error = Some(e),
                }
                cx.notify();
            });
        }));
    }

    pub fn copy(&mut self, cx: &mut Context<Self>) {
        let Some(f) = self.format() else { return };
        let text = build_aggregation_copy_text(&self.result, self.result_total, f);
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

    pub fn subtitle(&self) -> String {
        format!("{} · 表示中の絞り込みを反映", period_label(self.ctx.anchor, self.ctx.query.range == ReportRange::Month))
    }
}

fn button(id: &'static str) -> gpui::Stateful<gpui::Div> {
    div().id(id).debug_selector(move || id.into()).flex().items_center().gap(px(6.)).rounded_md().px_3().py(px(6.)).text_sm().cursor_pointer()
}

impl Render for CustomOutput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let format = self.format().cloned();
        let uses_ai = format.as_ref().is_some_and(report_copy_format_uses_ai);
        let provider = self.state.read(cx).data.as_ref().map(|d| d.config.ai_provider.clone()).unwrap_or_default();
        let format_label = |f: &ReportCopyFormat| format!("{}（{}・{}列）", f.name, if f.delimiter == "comma" { "CSV" } else { "TSV" }, f.columns.len());
        let body: gpui::AnyElement = if self.generating {
            let secs = self.started.map(|s| s.elapsed().as_secs()).unwrap_or(0);
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .text_sm()
                .text_color(n500())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Icon::new(IconName::LoaderCircle).size_5().text_color(gpui::rgb(0x1d4ed8)))
                        .child(if uses_ai { format!("{}で作成しています…", provider_label(&provider)) } else { "現在の集計から作成しています…".into() })
                        .when(uses_ai, |d| d.child(div().text_xs().text_color(n400()).child(elapsed_label(secs)))),
                )
                .when_some(self.progress.clone().filter(|_| uses_ai), |d, (kind, msg)| {
                    d.child(
                        div()
                            .id("ai-progress")
                            .max_h(px(160.))
                            .w_full()
                            .max_w(px(672.))
                            .overflow_y_scroll()
                            .rounded_lg()
                            .border_1()
                            .border_color(gpui::rgb(0xdbeafe))
                            .bg(with_alpha(gpui::rgb(0xeff6ff), 0.6))
                            .px_4()
                            .py_3()
                            .child(div().mb_1().text_size(px(11.)).font_weight(FontWeight::MEDIUM).text_color(gpui::rgb(0x1d4ed8)).child(if kind == "reasoning" { "考えていること" } else { "進捗" }))
                            .child(div().text_xs().text_color(n600()).child(msg)),
                    )
                })
                .into_any_element()
        } else if let Some(err) = &self.error {
            div().debug_selector(|| "output-error".into()).rounded_md().border_1().border_color(gpui::rgb(0xfecaca)).bg(red50()).px_4().py_3().text_sm().text_color(red700()).child(err.clone()).into_any_element()
        } else if !self.result.is_empty() {
            let f = format.clone().unwrap();
            let sum: i64 = self.result.iter().map(|r| r.minutes).sum();
            let count: i64 = self.result.iter().map(|r| r.entry_count).sum();
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .id("output-table")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .rounded_lg()
                        .border_1()
                        .border_color(n200())
                        .text_sm()
                        .child(div().flex().bg(n50()).text_color(n500()).font_weight(FontWeight::MEDIUM).children(f.columns.iter().map(|c| div().flex_1().px_3().py_2().child(report_copy_column_header(c)))))
                        .children(self.result.iter().map(|r| {
                            div().flex().border_t_1().border_color(n100()).text_color(n700()).children(f.columns.iter().map(|c| div().flex_1().px_3().py_2().child(aggregation_column_value(r, c, self.result_total))))
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_end()
                        .gap_x_5()
                        .text_xs()
                        .text_color(n500())
                        .when_some(self.restored_at, |d, at| {
                            let t = Local.timestamp_millis_opt(at).single().map(|t| t.format("%Y/%-m/%-d %-H:%M:%S").to_string()).unwrap_or_default();
                            d.child(div().debug_selector(|| "output-restored".into()).text_color(gpui::rgb(0x2563eb)).child(format!("保存済み結果を復元 · {t}")))
                        })
                        .child(format!("合計 {}", format_duration_report(sum)))
                        .child(format!("{count}件")),
                )
                .into_any_element()
        } else {
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .rounded_lg()
                .border_1()
                .border_dashed()
                .border_color(n200())
                .px_4()
                .py_10()
                .text_sm()
                .text_color(n400())
                .child("出力フォーマットを選んで「作成する」を押してください")
                .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .gap_4()
            .h(px(600.))
            .child(div().text_sm().text_color(n500()).child(self.subtitle()))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).text_color(n800()).child("出力形式"))
                    .map(|d| match &format {
                        None => d.child(div().rounded_md().border_1().border_color(gpui::rgb(0xfde68a)).bg(amber50()).px_3().py_2().text_sm().text_color(gpui::rgb(0x92400e)).child("設定の「レポート」で出力フォーマットを登録してください。")),
                        Some(f) => d.child(
                            div()
                                .relative()
                                .child(
                                    div()
                                        .id("format-select")
                                        .debug_selector(|| "format-select".into())
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(n300())
                                        .px_3()
                                        .py_2()
                                        .text_sm()
                                        .cursor_pointer()
                                        .child(format_label(f))
                                        .child(Icon::new(IconName::ChevronDown).size_3p5().text_color(n400()))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if !this.generating {
                                                this.picking_format = !this.picking_format;
                                                cx.notify();
                                            }
                                        })),
                                )
                                .when(self.picking_format, |d| {
                                    d.child(below(
                                        div()
                                            .mt_1()
                                            .w(px(420.))
                                            .rounded_md()
                                            .border_1()
                                            .border_color(n200())
                                            .bg(white())
                                            .shadow_lg()
                                            .py_1()
                                            .occlude()
                                            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                                                this.picking_format = false;
                                                cx.notify();
                                            }))
                                            .children(self.formats.iter().map(|f| {
                                                let id = f.id.clone();
                                                let sel = format!("format-{}", f.id);
                                                div()
                                                    .id(gpui::SharedString::from(sel.clone()))
                                                    .debug_selector(move || sel.clone())
                                                    .px_3()
                                                    .py(px(6.))
                                                    .text_sm()
                                                    .cursor_pointer()
                                                    .hover(|s| s.bg(n100()))
                                                    .child(format_label(f))
                                                    .on_click(cx.listener(move |this, _, _, cx| this.select_format(id.clone(), cx)))
                                            })),
                                    ))
                                }),
                        ),
                    })
                    .when(format.is_some(), |d| {
                        d.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_xs()
                                .text_color(n500())
                                .child(
                                    div()
                                        .rounded_full()
                                        .px_2()
                                        .py(px(2.))
                                        .font_weight(FontWeight::MEDIUM)
                                        .map(|d| if uses_ai { d.bg(gpui::rgb(0xede9fe)).text_color(gpui::rgb(0x6d28d9)) } else { d.bg(n100()).text_color(n600()) })
                                        .child(if uses_ai { "AI使用" } else { "AIなし" }),
                                )
                                .child(if uses_ai { "設定した指示で工数を再分類して出力します。" } else { "現在の集計結果を指定した列順で出力します。" }),
                        )
                    }),
            )
            .child(body)
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(button("output-close").hover(|s| s.bg(n100())).child("閉じる").on_click(cx.listener(|_, _, w, cx| w.close_dialog(cx))))
                    .child(
                        button("output-create")
                            .border_1()
                            .border_color(n300())
                            .map(|d| if self.generating || format.is_none() { d.opacity(0.5) } else { d.hover(|s| s.bg(n50())) })
                            .child(Icon::new(if self.generating { IconName::LoaderCircle } else { IconName::FileText }).size_4())
                            .child(if self.result.is_empty() { "作成する" } else { "再作成" })
                            .on_click(cx.listener(|this, _, _, cx| {
                                if !this.generating {
                                    this.create(cx)
                                }
                            })),
                    )
                    .when(!self.result.is_empty() && !self.generating, |d| {
                        d.child(
                            button("output-copy")
                                .bg(n900())
                                .text_color(white())
                                .hover(|s| s.bg(n700()))
                                .child(Icon::new(if self.copied { IconName::Check } else { IconName::Copy }).size_4())
                                .child(if self.copied { "コピーしました" } else { "結果をコピー" })
                                .on_click(cx.listener(|this, _, _, cx| this.copy(cx))),
                        )
                    }),
            )
    }
}

use gpui_component::WindowExt as _;

#[allow(unused)]
fn _assert_group_by(_: GroupBy) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(provider_label("codex"), "Codex");
        assert_eq!(provider_label("apple-intelligence"), "Apple Intelligence");
        assert_eq!(elapsed_label(65), "1:05");
    }
}
