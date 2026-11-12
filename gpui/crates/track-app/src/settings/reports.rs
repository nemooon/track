//! 「レポート」: output formats (Task 9) and the weekly report template (SettingsPage「週報テンプレート」).
use super::form::{self, Actions};
use crate::app::AppState;
use crate::ui::theme::*;
use gpui::{AppContext, Context, Entity, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, px};
use gpui_component::input::{Textarea, TextareaState};

pub struct ReportSettings {
    state: Entity<AppState>,
    pub template: Entity<TextareaState>,
    saving: bool,
    save_error: Option<String>,
    /// The saved value last put into the editor; differs from the editor once the user types.
    shown: String,
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
        ReportSettings { state, template, saving: false, save_error: None, shown: saved_copy }
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
        let value = self.template.read(cx).value().to_string();
        let saved = Self::saved_template(&self.state, cx);
        let dirty = value != saved;
        let error = if value.trim().is_empty() { Some("テンプレートを入力してください".to_string()) } else { self.save_error.clone() };
        let can_save = dirty && error.is_none() && !self.saving;
        div()
            .child(
                form::card("週報テンプレート", "AIが週の工数をまとめる際の出力構成を設定します。").child(form::row(
                    "出力テンプレート",
                    "Markdownの見出しや固定文を利用できます。",
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(div().h(px(300.)).child(Textarea::new(&self.template).h_full()))
                        .child(div().text_xs().text_color(n500()).child("Markdownの見出しや固定文をそのまま書けます。{{期間}} と {{合計時間}} は生成時に置き換わります。")),
                )),
            )
            .child(form::actions(
                Actions { prefix: "template", dirty, saving: self.saving, error: error.as_deref() },
                form::button("template-discard", "変更を破棄", false, !self.saving).on_click(cx.listener(|this, _, w, cx| this.reset(w, cx))),
                form::button("template-save", if self.saving { "保存中…" } else { "変更を保存" }, true, can_save).on_click(cx.listener(|this, _, _, cx| this.save(cx))),
            ))
    }
}
