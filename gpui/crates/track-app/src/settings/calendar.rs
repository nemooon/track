//! 「カレンダー」: working hours and days (SettingsPage「勤務スケジュール」).
use super::form::{self, Actions};
use crate::app::AppState;
use crate::ui::theme::*;
use gpui::{Context, Entity, InteractiveElement, IntoElement, ParentElement, Render, StatefulInteractiveElement, Styled, Window, div, px};

/// The time inputs step by 30 minutes, like `step={1800}` in the Tauri screen.
const STEP: i64 = 30;
const DAYS: [&str; 7] = ["日", "月", "火", "水", "木", "金", "土"];

pub struct CalendarSettings {
    state: Entity<AppState>,
    pub work_start: i64,
    pub work_end: i64,
    pub work_days: Vec<u32>,
    saving: bool,
    save_error: Option<String>,
    /// The user changed the draft; until then it follows the saved values.
    edited: bool,
}

impl CalendarSettings {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |this, _, cx| {
            if !this.edited {
                this.reset(cx);
            }
        })
        .detach();
        let mut s = CalendarSettings { state, work_start: 600, work_end: 1110, work_days: vec![], saving: false, save_error: None, edited: false };
        s.reset(cx);
        s
    }

    fn saved(&self, cx: &gpui::App) -> Option<(i64, i64, Vec<u32>)> {
        let d = self.state.read(cx).data.as_ref()?;
        let mut days = d.settings.work_days.clone();
        days.sort();
        Some((d.settings.work_start, d.settings.work_end, days))
    }

    pub fn reset(&mut self, cx: &mut Context<Self>) {
        if let Some((s, e, d)) = self.saved(cx) {
            (self.work_start, self.work_end, self.work_days) = (s, e, d);
        }
        self.save_error = None;
        self.edited = false;
        cx.notify();
    }

    pub fn dirty_in(&self, cx: &gpui::App) -> bool {
        let mut days = self.work_days.clone();
        days.sort();
        self.saved(cx).is_some_and(|saved| saved != (self.work_start, self.work_end, days))
    }

    /// The Tauri screen's client-side checks.
    pub fn validation(&self) -> Option<&'static str> {
        if self.work_end <= self.work_start {
            Some("勤務終了は勤務開始より後にしてください")
        } else if self.work_days.is_empty() {
            Some("勤務日を1日以上選択してください")
        } else {
            None
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        if self.validation().is_some() || self.saving {
            return;
        }
        self.saving = true;
        let mut days = self.work_days.clone();
        days.sort();
        let patch = track_core::model::SettingsPatch { work_start: Some(self.work_start), work_end: Some(self.work_end), work_days: Some(days), ..Default::default() };
        let task = self.state.update(cx, |s, cx| s.save_settings(patch, cx));
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                if result.is_ok() {
                    this.edited = false;
                }
                this.save_error = result.err().map(|e| e.message);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn stepper(&self, prefix: &'static str, value: i64, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let arrow = |id: String, label: &'static str, delta: i64, cx: &mut Context<Self>| {
            let sel = id.clone();
            div()
                .id(gpui::SharedString::from(id))
                .debug_selector(move || sel.clone())
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .text_color(p.text_muted)
                .hover(|s| s.bg(p.hover))
                .child(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    let slot = if prefix == "work-start" { &mut this.work_start } else { &mut this.work_end };
                    *slot = (*slot + delta).clamp(0, 1440);
                    this.edited = true;
                    cx.notify();
                }))
        };
        div()
            .flex()
            .items_center()
            .gap_1()
            .rounded_md()
            .border_1()
            .border_color(p.border)
            .child(arrow(format!("{prefix}-dec"), "−", -STEP, cx))
            .child(div().w(px(52.)).text_center().text_sm().font_family(MONO).child(form::minutes_label(value)))
            .child(arrow(format!("{prefix}-inc"), "+", STEP, cx))
    }
}

impl Render for CalendarSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let dirty = self.dirty_in(cx);
        let error = self.validation().map(str::to_string).or(self.save_error.clone());
        let can_save = dirty && self.validation().is_none() && !self.saving;
        let days = div().flex().gap_2().children(DAYS.iter().enumerate().map(|(i, label)| {
            let on = self.work_days.contains(&(i as u32));
            let id = format!("work-day-{i}");
            let sel = id.clone();
            // Several can be on: standalone chips the toolbar's height.
            crate::ui::controls::chip(&p, gpui::SharedString::from(id), on)
                .debug_selector(move || sel.clone())
                .child(*label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    let d = i as u32;
                    if let Some(p) = this.work_days.iter().position(|x| *x == d) {
                        this.work_days.remove(p);
                    } else {
                        this.work_days.push(d);
                        this.work_days.sort();
                    }
                    this.edited = true;
                    cx.notify();
                }))
        }));
        div()
            .child(
                form::section(p, "work-section", "勤務スケジュール", "", true)
                    .child(form::field(p, 
                        "work-field-hours",
                        "勤務時間",
                        "1日の標準的な開始・終了時刻です。",
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().text_xs().text_color(p.text_muted).child("勤務開始"))
                            .child(self.stepper("work-start", self.work_start, cx))
                            .child(div().text_xs().text_color(p.text_muted).child("勤務終了"))
                            .child(self.stepper("work-end", self.work_end, cx)),
                    ))
                    .child(form::field(p, "work-field-days", "勤務日", "勤務状況の判定対象にする曜日です。", days)),
            )
            .child(form::actions(p, 
                Actions { prefix: "work", dirty, saving: self.saving, error: error.as_deref() },
                form::button(p, "work-discard", "変更を破棄", false, !self.saving).on_click(cx.listener(|this, _, _, cx| this.reset(cx))),
                form::button(p, "work-save", if self.saving { "保存中…" } else { "変更を保存" }, true, can_save).on_click(cx.listener(|this, _, _, cx| this.save(cx))),
            ))
    }
}
