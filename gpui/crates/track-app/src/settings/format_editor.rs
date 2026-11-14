//! Editing one custom-output format (ReportCopyFormatPage.tsx): name, delimiter, header row, the
//! AI instruction and the ordered columns. Formats always target the custom output (aggregation).
use super::form;
use crate::app::AppState;
use crate::ui::theme::*;
use gpui::{
    AnyElement, AppContext, Context, Div, Entity, EventEmitter, FontWeight, InteractiveElement, IntoElement, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::input::{Input, InputState, Textarea, TextareaState};
use gpui_component::{Icon, IconName};
use std::collections::HashMap;
use track_core::model::{ReportCopyColumn, ReportCopyField, ReportCopyFormat, ReportDurationFormat, SettingsPatch};
use track_core::report::copy::{field_label, report_copy_format_uses_ai};

/// `AI_REPORT_COPY_FIELDS`
const FIELDS: [ReportCopyField; 6] =
    [ReportCopyField::Category, ReportCopyField::Summary, ReportCopyField::Duration, ReportCopyField::DurationMinutes, ReportCopyField::Percentage, ReportCopyField::EntryCount];

const DURATION_FORMATS: [(ReportDurationFormat, &str, &str); 4] = [
    (ReportDurationFormat::HoursMinutes, "hours-minutes", "1:30"),
    (ReportDurationFormat::Japanese, "japanese", "1時間30分"),
    (ReportDurationFormat::DecimalWithUnit, "decimal-with-unit", "1.5時間"),
    (ReportDurationFormat::Decimal, "decimal", "1.5（単位なし）"),
];

/// The new format created by 「出力フォーマットを追加」.
pub fn new_format() -> ReportCopyFormat {
    ReportCopyFormat {
        id: track_core::ids::cuid(),
        name: "新しい出力形式".into(),
        target: "ai-aggregation".into(),
        delimiter: "tab".into(),
        include_header: true,
        ai_prompt: String::new(),
        columns: [ReportCopyField::Category, ReportCopyField::Duration, ReportCopyField::Percentage].into_iter().map(field_column).collect(),
    }
}

fn field_column(field: ReportCopyField) -> ReportCopyColumn {
    let duration_format = (field == ReportCopyField::Duration).then_some(ReportDurationFormat::HoursMinutes);
    ReportCopyColumn::Field { id: track_core::ids::cuid(), field, label: None, duration_format }
}

fn column_kind_label(c: &ReportCopyColumn) -> &'static str {
    match c {
        ReportCopyColumn::Blank { .. } => "空白",
        ReportCopyColumn::Ai { .. } => "AI生成",
        ReportCopyColumn::Field { field, .. } => field_label(*field),
    }
}

/// `copyFormatError`
pub fn format_error(f: &ReportCopyFormat) -> Option<&'static str> {
    if f.name.trim().is_empty() {
        Some("出力フォーマットの名前を入力してください")
    } else if f.columns.is_empty() {
        Some("項目を1つ以上追加してください")
    } else if f.columns.iter().any(|c| matches!(c, ReportCopyColumn::Ai { label, prompt, .. } if label.trim().is_empty() || prompt.trim().is_empty())) {
        Some("AI生成項目の見出しと生成指示を入力してください")
    } else {
        None
    }
}

/// Inputs of one column: the header, and the instruction for AI columns.
struct ColumnInputs {
    header: Entity<InputState>,
    prompt: Option<Entity<InputState>>,
}

/// Emitted when the editor should close (back, saved-and-deleted).
pub struct CloseEditor;

pub struct FormatEditor {
    state: Entity<AppState>,
    /// The structure being edited; text fields live in the inputs until `current()`.
    draft: ReportCopyFormat,
    pub name: Entity<InputState>,
    ai_prompt: Entity<TextareaState>,
    inputs: HashMap<String, ColumnInputs>,
    confirm_delete: bool,
    saving: bool,
    save_error: Option<String>,
}

impl EventEmitter<CloseEditor> for FormatEditor {}

impl FormatEditor {
    pub fn new(state: Entity<AppState>, format: ReportCopyFormat, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).default_value(format.name.clone()));
        let ai_prompt = cx.new(|cx| TextareaState::new(window, cx).rows(3).placeholder("例: 作業の目的と内容が同じものをまとめる").default_value(format.ai_prompt.clone()));
        cx.observe(&name, |_, _, cx| cx.notify()).detach();
        cx.observe(&ai_prompt, |_, _, cx| cx.notify()).detach();
        let mut ed = FormatEditor { state, draft: format, name, ai_prompt, inputs: HashMap::new(), confirm_delete: false, saving: false, save_error: None };
        for c in ed.draft.columns.clone() {
            ed.add_inputs(&c, window, cx);
        }
        ed
    }

    fn add_inputs(&mut self, c: &ReportCopyColumn, window: &mut Window, cx: &mut Context<Self>) {
        let (label, prompt) = match c {
            ReportCopyColumn::Field { label, .. } | ReportCopyColumn::Blank { label, .. } => (label.clone().unwrap_or_default(), None),
            ReportCopyColumn::Ai { label, prompt, .. } => (label.clone(), Some(prompt.clone())),
        };
        let placeholder = match c {
            ReportCopyColumn::Field { field, .. } => field_label(*field).to_string(),
            _ => "見出しなし".to_string(),
        };
        let header = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(label));
        cx.observe(&header, |_, _, cx| cx.notify()).detach();
        let prompt = prompt.map(|p| {
            let i = cx.new(|cx| InputState::new(window, cx).placeholder("生成指示").default_value(p));
            cx.observe(&i, |_, _, cx| cx.notify()).detach();
            i
        });
        self.inputs.insert(c.id().to_string(), ColumnInputs { header, prompt });
    }

    /// The draft with the text inputs applied (not yet trimmed).
    fn current(&self, cx: &gpui::App) -> ReportCopyFormat {
        let mut f = self.draft.clone();
        f.name = self.name.read(cx).value().to_string();
        f.ai_prompt = self.ai_prompt.read(cx).value().to_string();
        for c in &mut f.columns {
            let Some(inp) = self.inputs.get(c.id()) else { continue };
            let header = inp.header.read(cx).value().to_string();
            match c {
                ReportCopyColumn::Field { label, .. } | ReportCopyColumn::Blank { label, .. } => *label = (!header.is_empty()).then_some(header),
                ReportCopyColumn::Ai { label, prompt, .. } => {
                    *label = header;
                    if let Some(p) = &inp.prompt {
                        *prompt = p.read(cx).value().to_string();
                    }
                }
            }
        }
        f
    }

    fn saved(&self, cx: &gpui::App) -> Option<ReportCopyFormat> {
        self.state.read(cx).data.as_ref()?.settings.report_copy_formats.iter().find(|x| x.id == self.draft.id).cloned()
    }

    fn all_saved(&self, cx: &gpui::App) -> Vec<ReportCopyFormat> {
        self.state.read(cx).data.as_ref().map(|d| d.settings.report_copy_formats.clone()).unwrap_or_default()
    }

    fn insert_column(&mut self, c: ReportCopyColumn, window: &mut Window, cx: &mut Context<Self>) {
        self.add_inputs(&c, window, cx);
        self.draft.columns.push(c);
        cx.notify();
    }

    fn move_column(&mut self, index: usize, up: bool, cx: &mut Context<Self>) {
        let to = if up { index.checked_sub(1) } else { Some(index + 1).filter(|i| *i < self.draft.columns.len()) };
        if let Some(to) = to {
            self.draft.columns.swap(index, to);
            cx.notify();
        }
    }

    fn write(&mut self, formats: Vec<ReportCopyFormat>, close: bool, cx: &mut Context<Self>) {
        self.saving = true;
        let task = self.state.update(cx, |s, cx| s.save_settings(SettingsPatch { report_copy_formats: Some(formats), ..Default::default() }, cx));
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                match result {
                    Ok(_) if close => cx.emit(CloseEditor),
                    Ok(_) => this.save_error = None,
                    Err(e) => this.save_error = Some(e.message),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let current = self.current(cx);
        if format_error(&current).is_some() || self.saving {
            return;
        }
        let formats = self.all_saved(cx).into_iter().map(|f| if f.id == current.id { current.clone() } else { f }).collect();
        self.write(formats, false, cx);
    }

    fn delete(&mut self, cx: &mut Context<Self>) {
        let formats: Vec<_> = self.all_saved(cx).into_iter().filter(|f| f.id != self.draft.id).collect();
        if formats.is_empty() {
            return;
        }
        self.confirm_delete = false;
        self.write(formats, true, cx);
    }

    fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(saved) = self.saved(cx) {
            *self = FormatEditor::new(self.state.clone(), saved, window, cx);
        }
        cx.notify();
    }

    fn render_column(&self, index: usize, c: &ReportCopyColumn, count: usize, cx: &mut Context<Self>) -> AnyElement {
        let p = palette(cx);
        let small = |id: String, icon: IconName, enabled: bool| {
            let sel = id.clone();
            div()
                .id(SharedString::from(id))
                .debug_selector(move || sel.clone())
                .p_1()
                .rounded_md()
                .text_color(p.text_muted)
                .when(enabled, |d| d.cursor_pointer().hover(|s| s.bg(p.hover).text_color(p.text)))
                .when(!enabled, |d| d.opacity(0.3))
                .child(Icon::new(icon).size_4())
        };
        let inputs = self.inputs.get(c.id());
        let mut row = div()
            .flex()
            .flex_col()
            .gap_2()
            .py_3()
            .border_b_1()
            .border_color(p.border_subtle)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(div().w(px(70.)).flex_shrink_0().text_xs().font_weight(FontWeight::MEDIUM).text_color(p.text_muted).child(column_kind_label(c)))
                    .when_some(inputs.map(|i| i.header.clone()), |d, h| d.child(div().flex_1().child(Input::new(&h))))
                    .child(small(format!("column-up-{index}"), IconName::ArrowUp, index > 0).on_click(cx.listener(move |this, _, _, cx| this.move_column(index, true, cx))))
                    .child(small(format!("column-down-{index}"), IconName::ArrowDown, index + 1 < count).on_click(cx.listener(move |this, _, _, cx| this.move_column(index, false, cx))))
                    .child(small(format!("column-remove-{index}"), IconName::Close, true).on_click(cx.listener(move |this, _, _, cx| {
                        let removed = this.draft.columns.remove(index);
                        this.inputs.remove(removed.id());
                        cx.notify();
                    }))),
            );
        if let ReportCopyColumn::Field { field: ReportCopyField::Duration, duration_format, .. } = c {
            let current = duration_format.unwrap_or(ReportDurationFormat::HoursMinutes);
            row = row.child(div().pl(px(82.)).flex().flex_wrap().gap_2().children(DURATION_FORMATS.iter().map(|(fmt, key, label)| {
                let fmt = *fmt;
                let id = format!("duration-format-{index}-{key}");
                let sel = id.clone();
                div()
                    .id(SharedString::from(id))
                    .debug_selector(move || sel.clone())
                    .px_2()
                    .py(px(2.))
                    .rounded_md()
                    .text_xs()
                    .cursor_pointer()
                    .when(current == fmt, |d| d.bg(p.primary).text_color(p.on_color))
                    .when(current != fmt, |d| d.border_1().border_color(p.border).text_color(p.text_muted))
                    .child(*label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(ReportCopyColumn::Field { duration_format, .. }) = this.draft.columns.get_mut(index) {
                            *duration_format = Some(fmt);
                        }
                        cx.notify();
                    }))
            })));
        }
        if let Some(p) = inputs.and_then(|i| i.prompt.clone()) {
            row = row.child(div().pl(px(82.)).child(Input::new(&p)));
        }
        row.into_any_element()
    }

    fn add_buttons(&self, cx: &mut Context<Self>) -> Div {
        let p = palette(cx);
        let chip = |id: String, label: String| {
            let sel = id.clone();
            div()
                .id(SharedString::from(id))
                .debug_selector(move || sel.clone())
                .px_2()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(p.border)
                .text_xs()
                .cursor_pointer()
                .hover(|s| s.bg(p.hover))
                .child(label)
        };
        let key = |f: ReportCopyField| serde_json::to_value(f).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .pt_1()
            .child(div().text_xs().text_color(p.text_muted).child("項目を追加:"))
            .children(FIELDS.iter().map(|f| {
                let f = *f;
                chip(format!("add-column-{}", key(f)), field_label(f).to_string()).on_click(cx.listener(move |this, _, w, cx| this.insert_column(field_column(f), w, cx)))
            }))
            .child(chip("add-column-blank".into(), "空白".into()).on_click(cx.listener(|this, _, w, cx| {
                this.insert_column(ReportCopyColumn::Blank { id: track_core::ids::cuid(), label: None }, w, cx)
            })))
            .child(chip("add-column-ai".into(), "AI生成".into()).on_click(cx.listener(|this, _, w, cx| {
                this.insert_column(
                    ReportCopyColumn::Ai { id: track_core::ids::cuid(), label: "AI生成項目".into(), prompt: "この集計の内容を簡潔にまとめる".into() },
                    w,
                    cx,
                )
            })))
    }
}

impl Render for FormatEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let current = self.current(cx);
        let saved = self.saved(cx);
        let dirty = saved.as_ref() != Some(&current);
        let error = format_error(&current).map(str::to_string).or(self.save_error.clone());
        let can_save = dirty && format_error(&current).is_none() && !self.saving;
        let can_delete = self.all_saved(cx).len() > 1;
        let segment = |id: &'static str, label: &'static str, on: bool| crate::ui::controls::button(&p, id, on, false).debug_selector(move || id.into()).child(label);
        let toggle = |id: &'static str, label: &'static str, on: bool| crate::ui::controls::chip(&p, id, on).debug_selector(move || id.into()).child(label);
        let comma = current.delimiter == "comma";
        let count = current.columns.len();
        let columns: Vec<AnyElement> = current.columns.iter().enumerate().map(|(i, c)| self.render_column(i, c, count, cx)).collect();
        div()
            .child(
                div()
                    .id("format-back")
                    .debug_selector(|| "format-back".into())
                    .flex()
                    .items_center()
                    .gap_1()
                    .mb_4()
                    .text_sm()
                    .text_color(p.text_muted)
                    .cursor_pointer()
                    .hover(|s| s.text_color(p.text))
                    .child(Icon::new(IconName::ChevronLeft).size_4())
                    .child("出力フォーマット")
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(CloseEditor))),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .mb_4()
                    .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(if current.name.trim().is_empty() { "名前未設定".to_string() } else { current.name.clone() }))
                    .child(div().px_2().rounded_md().text_xs().bg(p.hover).text_color(p.text_muted).child(if report_copy_format_uses_ai(&current) { "AI使用" } else { "AIなし" })),
            )
            .child(
                form::section(p, "format-section-basics", "基本設定", "", true)
                    .child(form::field(p, "format-field-name", "名前", "", div().w(px(320.)).child(Input::new(&self.name))))
                    .child(form::field(p, 
                        "format-field-delimiter",
                        "区切り文字",
                        "",
                        div().flex().child(crate::ui::controls::group(&p)
                            .child(segment("delimiter-tab", "タブ（TSV）", !comma).on_click(cx.listener(|this, _, _, cx| {
                                this.draft.delimiter = "tab".into();
                                cx.notify();
                            })))
                            .child(segment("delimiter-comma", "カンマ（CSV）", comma).on_click(cx.listener(|this, _, _, cx| {
                                this.draft.delimiter = "comma".into();
                                cx.notify();
                            })))),
                    ))
                    .child(form::field(p, 
                        "format-field-header",
                        "見出し行",
                        "",
                        toggle("include-header", "先頭行に項目名を含める", current.include_header).on_click(cx.listener(|this, _, _, cx| {
                            this.draft.include_header = !this.draft.include_header;
                            cx.notify();
                        })),
                    )),
            )
            .child(
                form::section(p, "format-section-ai", "AIへの指示", "集計全体への指示です。空欄でもAI生成項目や概要項目があればAIを使用します。", false)
                    .child(div().h(px(90.)).child(Textarea::new(&self.ai_prompt).h_full())),
            )
            .child(
                form::section(p, "format-section-columns", "出力項目", "追加後も見出しを変更できます。上下の矢印で出力順を変更します。", false)
                    // The list is ruled off above and between its rows (no box around it).
                    .child(div().flex().flex_col().border_t_1().border_color(p.border_subtle).children(columns))
                    .child(self.add_buttons(cx)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .mb_8()
                    .child({
                        let button = form::button(p, "format-delete", if can_delete { "このフォーマットを削除" } else { "最後の出力フォーマットは削除できません" }, false, can_delete);
                        if can_delete {
                            // Confirms in a popover from the button.
                            let (this, this2) = (cx.entity().downgrade(), cx.entity().downgrade());
                            super::popover::confirm(
                                "format-delete-pop",
                                button,
                                self.confirm_delete,
                                move |open, _, cx| {
                                    let _ = this.update(cx, |this, cx| {
                                        this.confirm_delete = open;
                                        cx.notify();
                                    });
                                },
                                super::popover::Confirm {
                                    message: format!("「{}」を削除しますか？", current.name),
                                    cancel_id: "format-delete-cancel",
                                    confirm_id: "format-delete-confirm",
                                    confirm_label: "削除",
                                },
                                move |_, cx| {
                                    let _ = this2.update(cx, |this, cx| this.delete(cx));
                                },
                                p,
                            )
                            .into_any_element()
                        } else {
                            button.into_any_element()
                        }
                    })
                    .child(div().flex_1().child(form::actions(p, 
                        form::Actions { prefix: "format", dirty, saving: self.saving, error: error.as_deref() },
                        form::button(p, "format-discard", "変更を破棄", false, !self.saving).on_click(cx.listener(|this, _, w, cx| this.discard(w, cx))),
                        form::button(p, "format-save", if self.saving { "保存中…" } else { "変更を保存" }, true, can_save).on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                    ))),
            )
    }
}
