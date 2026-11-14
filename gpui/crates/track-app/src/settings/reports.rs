//! 「レポート」: output formats (Task 9) and the weekly report template (SettingsPage「週報テンプレート」).
use super::form::{self, Actions};
use crate::app::AppState;
use crate::ui::theme::*;
use super::format_editor::{self, CloseEditor, FormatEditor};
use gpui::{
    AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    Subscription, Window, div, prelude::FluentBuilder, px,
};
use track_core::model::{ReportCopyFormat, SettingsPatch};
use track_core::report::copy::report_copy_format_uses_ai;
use gpui_component::input::{Textarea, TextareaState};

pub struct ReportSettings {
    state: Entity<AppState>,
    pub template: Entity<TextareaState>,
    saving: bool,
    save_error: Option<String>,
    /// The saved value last put into the editor; differs from the editor once the user types.
    shown: String,
    /// The format being edited, replacing the list (Tauri navigates to /report-formats/:id).
    pub editing: Option<Entity<FormatEditor>>,
    _editing_sub: Option<Subscription>,
    adding: bool,
}

impl ReportSettings {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let saved = Self::saved_template(&state, cx);
        let saved_copy = saved.clone();
        let template = cx.new(|cx| TextareaState::new(window, cx).rows(14).default_value(saved));
        cx.observe(&template, |_, _, cx| cx.notify()).detach();
        // An untouched editor follows the saved template (it may load after the window opens).
        cx.observe_in(&state, window, |this: &mut Self, _, window, cx| {
            let value = this.template.read(cx).value().to_string();
            let saved = Self::saved_template(&this.state, cx);
            if value == this.shown && value != saved {
                this.shown = saved.clone();
                this.template.update(cx, |t, cx| t.set_value(saved, window, cx));
            }
        })
        .detach();
        ReportSettings { state, template, saving: false, save_error: None, shown: saved_copy, editing: None, _editing_sub: None, adding: false }
    }

    fn saved_template(state: &Entity<AppState>, cx: &gpui::App) -> String {
        state.read(cx).data.as_ref().map(|d| d.settings.weekly_report_template.clone()).unwrap_or_default()
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let saved = Self::saved_template(&self.state, cx);
        self.shown = saved.clone();
        self.template.update(cx, |t, cx| t.set_value(saved, window, cx));
        self.save_error = None;
        cx.notify();
    }

    fn formats(&self, cx: &gpui::App) -> Vec<ReportCopyFormat> {
        self.state.read(cx).data.as_ref().map(|d| d.settings.report_copy_formats.clone()).unwrap_or_default()
    }

    fn open_format(&mut self, format: ReportCopyFormat, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.state.clone();
        let editor = cx.new(|cx| FormatEditor::new(state, format, window, cx));
        self._editing_sub = Some(cx.subscribe(&editor, |this, _, _: &CloseEditor, cx| {
            this.editing = None;
            this._editing_sub = None;
            cx.notify();
        }));
        self.editing = Some(editor);
        cx.notify();
    }

    /// Like Tauri: the new format is saved first, then opened.
    fn add_format(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut formats = self.formats(cx);
        if formats.len() >= 20 || self.adding {
            return;
        }
        let new = format_editor::new_format();
        formats.push(new.clone());
        self.adding = true;
        let task = self.state.update(cx, |s, cx| s.save_settings(SettingsPatch { report_copy_formats: Some(formats), ..Default::default() }, cx));
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.adding = false;
                match result {
                    Ok(_) => this.open_format(new, window, cx),
                    Err(e) => this.save_error = Some(e.message),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let value = self.template.read(cx).value().to_string();
        if value.trim().is_empty() || self.saving {
            return;
        }
        self.saving = true;
        let patch = track_core::model::SettingsPatch { weekly_report_template: Some(value), ..Default::default() };
        let task = self.state.update(cx, |s, cx| s.save_settings(patch, cx));
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                if let Ok(s) = &result {
                    this.shown = s.weekly_report_template.clone();
                }
                this.save_error = result.err().map(|e| e.message);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for ReportSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        if let Some(editor) = &self.editing {
            return div().child(editor.clone());
        }
        let formats = self.formats(cx);
        let rows = formats.iter().enumerate().map(|(i, f)| {
            let id = format!("format-row-{i}");
            let sel = id.clone();
            let open = f.clone();
            let ai = report_copy_format_uses_ai(f);
            div()
                .id(SharedString::from(id))
                .debug_selector(move || sel.clone())
                .flex()
                .items_center()
                .gap_3()
                .px_5()
                .py_3()
                .border_b_1()
                .border_color(p.hover)
                .cursor_pointer()
                .hover(|s| s.bg(p.surface))
                .child(div().flex_1().text_sm().font_weight(FontWeight::MEDIUM).child(if f.name.is_empty() { "名前未設定".to_string() } else { f.name.clone() }))
                .child(
                    div()
                        .px_2()
                        .rounded_md()
                        .text_xs()
                        .when(ai, |d| d.bg(p.ai_bg).text_color(p.ai))
                        .when(!ai, |d| d.bg(p.hover).text_color(p.text_muted))
                        .child(if ai { "AI使用" } else { "AIなし" }),
                )
                .child(div().text_xs().text_color(p.text_muted).child(format!("{}列 · {}", f.columns.len(), if f.delimiter == "comma" { "CSV" } else { "TSV" })))
                .on_click(cx.listener(move |this, _, w, cx| this.open_format(open.clone(), w, cx)))
        });
        let can_add = formats.len() < 20 && !self.adding;
        let formats_card = form::card(p, "出力フォーマット", "フォーマットを選択すると、専用の編集画面を開きます。")
            .children(rows)
            .child(div().px_5().py_3().child(form::button(p, "format-add", "出力フォーマットを追加", false, can_add).when(can_add, |d| {
                d.on_click(cx.listener(|this, _, w, cx| this.add_format(w, cx)))
            })));
        let value = self.template.read(cx).value().to_string();
        let saved = Self::saved_template(&self.state, cx);
        let dirty = value != saved;
        let error = if value.trim().is_empty() { Some("テンプレートを入力してください".to_string()) } else { self.save_error.clone() };
        let can_save = dirty && error.is_none() && !self.saving;
        div()
            .child(formats_card)
            .child(
                form::card(p, "週報テンプレート", "AIが週の工数をまとめる際の出力構成を設定します。").child(form::row(p, 
                    "出力テンプレート",
                    "Markdownの見出しや固定文を利用できます。",
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(div().h(px(300.)).child(Textarea::new(&self.template).h_full()))
                        .child(div().text_xs().text_color(p.text_muted).child("Markdownの見出しや固定文をそのまま書けます。{{期間}} と {{合計時間}} は生成時に置き換わります。")),
                )),
            )
            .child(form::actions(p, 
                Actions { prefix: "template", dirty, saving: self.saving, error: error.as_deref() },
                form::button(p, "template-discard", "変更を破棄", false, !self.saving).on_click(cx.listener(|this, _, w, cx| this.reset(w, cx))),
                form::button(p, "template-save", if self.saving { "保存中…" } else { "変更を保存" }, true, can_save).on_click(cx.listener(|this, _, _, cx| this.save(cx))),
            ))
    }
}
