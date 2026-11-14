//! 「タグ」: one row per tag, edited in place. Double-click a row to rename it (Enter saves; Esc,
//! leaving the field or clicking elsewhere cancels), press its colour dot for the colours, and the
//! trash icon (shown on hover) deletes it after a confirmation. Each row says how many projects and
//! entries use the tag. Everything saves at once; there is no form.
use super::form;
use crate::app::AppState;
use crate::data::{DataClient, DataResult};
use crate::ui::theme::*;
use gpui::{
    AppContext, Context, Div, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
    Window, div, prelude::FluentBuilder, px,
};
use gpui_component::color_picker::{ColorPickerEvent, ColorPickerState};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{Icon, IconName, Sizable};
use std::collections::HashMap;
use std::future::Future;
use track_core::model::{Tag, TagPatch};

/// The tag colours (shared with projects and track-cli).
pub use track_core::edit::PROJECT_COLORS as COLORS;

pub struct TagSettings {
    state: Entity<AppState>,
    pub tags: Vec<Tag>,
    /// Tag id → (projects, entries) using it.
    usage: HashMap<String, (i64, i64)>,
    /// The tag being renamed, with its field.
    pub editing: Option<(String, Entity<InputState>)>,
    /// A tag just added: rename it once it has been read.
    rename_next: Option<String>,
    /// The tag whose colours are open under its row, and the eyedropper for it.
    colors_for: Option<String>,
    picker: Option<Entity<ColorPickerState>>,
    pending_color: Option<(String, gpui::Hsla)>,
    /// The tag whose delete is being confirmed.
    deleting: Option<String>,
    /// A failed save shown under that tag's row.
    row_error: Option<(String, String)>,
    error: Option<String>,
}

fn api(state: &Entity<AppState>, cx: &gpui::App) -> DataClient {
    state.read(cx).api.clone()
}

impl TagSettings {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        // Restores and outside writes (CLI, AI) re-read everything; follow them.
        cx.subscribe(&state, |this, _, _: &crate::app::StateEvent, cx| this.reload(cx)).detach();
        let mut s = TagSettings {
            state,
            tags: vec![],
            usage: HashMap::new(),
            editing: None,
            rename_next: None,
            colors_for: None,
            picker: None,
            pending_color: None,
            deleting: None,
            row_error: None,
            error: None,
        };
        s.reload(cx);
        s
    }

    /// Re-reads the tags and how much each is used.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let api = api(&self.state, cx);
        let (t, u) = (api.tags(), api.tag_usage());
        cx.spawn(async move |this, cx| {
            let result = async { Ok::<_, crate::data::DataError>((t.await?, u.await?)) }.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok((t, u)) => {
                        this.tags = t;
                        this.usage = u.into_iter().map(|x| (x.tag_id, (x.projects, x.entries))).collect();
                        // Whatever was open for a tag that no longer exists closes.
                        let exists = |id: &str| this.tags.iter().any(|t| t.id == id);
                        if this.editing.as_ref().is_some_and(|(id, _)| !exists(id)) {
                            this.editing = None;
                        }
                        if this.colors_for.as_deref().is_some_and(|id| !exists(id)) {
                            (this.colors_for, this.picker) = (None, None);
                        }
                    }
                    Err(e) => this.error = Some(e.message),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Runs a write for one tag. Success refreshes everything (this page re-reads through the
    /// state event); failure shows the message under that tag's row.
    fn write<T: Send + 'static>(&mut self, tag_id: String, fut: impl Future<Output = DataResult<T>> + Send + 'static, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(_) => {
                        this.row_error = None;
                        this.state.update(cx, |s, cx| s.refresh_all(cx));
                    }
                    Err(e) => {
                        let message = match e.code.as_str() {
                            "tag_already_exists" => "同じ名前のタグが既にあります".to_string(),
                            "invalid_input" => "名前は50文字以内で入力してください".to_string(),
                            _ => e.message,
                        };
                        this.row_error = Some((tag_id, message));
                        this.reload(cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// 「2 プロジェクト・14 件」.
    pub fn usage_label(&self, tag_id: &str) -> String {
        let (projects, entries) = self.usage.get(tag_id).copied().unwrap_or((0, 0));
        format!("{projects} プロジェクト・{entries} 件")
    }

    /// Adds 「新しいタグ」 (numbered when taken) and starts renaming it.
    fn add_tag(&mut self, cx: &mut Context<Self>) {
        let taken = |n: &str| self.tags.iter().any(|t| t.name == n);
        let name = std::iter::once("新しいタグ".to_string()).chain((2..).map(|i| format!("新しいタグ {i}"))).find(|n| !taken(n)).unwrap();
        let color = track_core::edit::next_project_color(self.tags.len()).to_string();
        let fut = api(&self.state, cx).create_tag(name, color);
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(id) => {
                        this.rename_next = Some(id);
                        this.error = None;
                        this.state.update(cx, |s, cx| s.refresh_all(cx));
                    }
                    Err(e) => this.error = Some(e.message),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Opens the tag's name field (closing anything else that was open).
    fn start_rename(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.as_ref().is_some_and(|(e, _)| *e == id) {
            return;
        }
        let Some(name) = self.tags.iter().find(|t| t.id == id).map(|t| t.name.clone()) else { return };
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("タグ名").default_value(name));
        cx.subscribe_in(&input, window, |this, _, ev: &InputEvent, _, cx| {
            // Only Enter saves; leaving the field cancels (so do Escape and clicking elsewhere).
            match ev {
                InputEvent::PressEnter { .. } => this.commit_rename(cx),
                InputEvent::Blur => this.cancel_rename(cx),
                _ => {}
            }
        })
        .detach();
        input.update(cx, |i, cx| i.focus(window, cx));
        (self.deleting, self.row_error) = (None, None);
        self.editing = Some((id, input));
        cx.notify();
    }

    /// Saves the name being edited; an empty or unchanged name saves nothing.
    fn commit_rename(&mut self, cx: &mut Context<Self>) {
        let Some((id, input)) = self.editing.take() else { return };
        let value = input.read(cx).value().trim().to_string();
        if !value.is_empty() && self.tags.iter().any(|t| t.id == id && t.name != value) {
            let fut = api(&self.state, cx).update_tag(id.clone(), TagPatch { name: Some(value), color: None });
            self.write(id, fut, cx);
        }
        cx.notify();
    }

    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        if self.editing.take().is_some() {
            cx.notify();
        }
    }

    fn save_color(&mut self, id: String, color: String, cx: &mut Context<Self>) {
        let fut = api(&self.state, cx).update_tag(id.clone(), TagPatch { name: None, color: Some(color) });
        self.write(id, fut, cx);
    }

    fn save_pending_color(&mut self, cx: &mut Context<Self>) {
        if let Some((id, c)) = self.pending_color.take() {
            self.save_color(id, to_hex(c), cx);
        }
    }

    /// Opens (or closes) the colours under a tag's row, with an eyedropper set to its colour.
    fn toggle_colors(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.save_pending_color(cx);
        if self.colors_for.as_deref() == Some(id.as_str()) {
            (self.colors_for, self.picker) = (None, None);
            cx.notify();
            return;
        }
        let Some(color) = self.tags.iter().find(|t| t.id == id).map(|t| t.color.clone()) else { return };
        let picker = cx.new(|cx| ColorPickerState::new(window, cx).default_value(hex(&color)));
        let tid = id.clone();
        cx.subscribe(&picker, move |this, _, ev: &ColorPickerEvent, _| {
            if let ColorPickerEvent::Change(Some(c)) = ev {
                this.pending_color = Some((tid.clone(), *c));
            }
        })
        .detach();
        // The eyedropper saves when its popover closes, not on every slider step.
        cx.observe(&picker, |this, picker, cx| {
            if !picker.read(cx).is_open() && this.pending_color.is_some() {
                this.save_pending_color(cx);
                (this.colors_for, this.picker) = (None, None);
            }
        })
        .detach();
        self.cancel_rename(cx);
        (self.colors_for, self.picker) = (Some(id), Some(picker));
        cx.notify();
    }

    fn render_row(&self, t: &Tag, p: Palette, cx: &mut Context<Self>) -> Div {
        let group = SharedString::from(format!("tag-row-group-{}", t.id));
        let (id, dot_id, del_id) = (t.id.clone(), t.id.clone(), t.id.clone());
        let editing = self.editing.as_ref().filter(|(e, _)| *e == t.id).map(|(_, i)| i.clone());
        let name = match editing {
            Some(input) => div()
                .flex_1()
                .min_w_0()
                .on_action(cx.listener(|this, _: &gpui_component::input::Escape, _, cx| this.cancel_rename(cx)))
                .on_mouse_down_out(cx.listener(|this, _, _, cx| this.cancel_rename(cx)))
                .child(Input::new(&input).small())
                .into_any_element(),
            None => div().flex_1().min_w_0().truncate().child(t.name.clone()).into_any_element(),
        };
        let row_sel = format!("tag-row-{}", t.id);
        let row_sel2 = row_sel.clone();
        let dot_sel = format!("tag-color-{}", t.id);
        let dot_sel2 = dot_sel.clone();
        let usage_sel = format!("tag-usage-{}", t.id);
        let del_sel = format!("tag-delete-{}", t.id);
        let del_sel2 = del_sel.clone();
        let confirming = self.deleting.as_deref() == Some(t.id.as_str());
        let row = div()
            .id(SharedString::from(row_sel))
            .debug_selector(move || row_sel2.clone())
            .group(group.clone())
            .flex()
            .items_center()
            .gap_3()
            .h(px(40.))
            .px_3()
            .rounded(px(6.))
            .text_sm()
            .hover(move |s| s.bg(p.hover))
            .child(
                div()
                    .id(SharedString::from(dot_sel))
                    .debug_selector(move || dot_sel2.clone())
                    .size(px(14.))
                    .flex_none()
                    .rounded_full()
                    .cursor_pointer()
                    .bg(hex(&t.color))
                    .when(self.colors_for.as_deref() == Some(t.id.as_str()), |d| d.border_2().border_color(p.text))
                    .tooltip(|window, cx| gpui_component::tooltip::Tooltip::new("色を変える").build(window, cx))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.toggle_colors(dot_id.clone(), window, cx)
                    })),
            )
            .child(name)
            .child(div().debug_selector(move || usage_sel.clone()).flex_none().text_xs().text_color(p.text_faint).child(self.usage_label(&t.id)))
            .child(
                div()
                    .id(SharedString::from(del_sel))
                    .debug_selector(move || del_sel2.clone())
                    .p_1()
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_color(p.text_muted)
                    .hover(move |s| s.bg(p.hover).text_color(p.text))
                    // Shown while the row is hovered (or its delete is being confirmed).
                    .opacity(if confirming { 1. } else { 0. })
                    .group_hover(group, |s| s.opacity(1.))
                    .child(crate::ui::icons::trash().size_4())
                    .tooltip(|window, cx| gpui_component::tooltip::Tooltip::new("削除する").build(window, cx))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.cancel_rename(cx);
                        this.deleting = Some(del_id.clone());
                        cx.notify();
                    })),
            )
            // Double-click renames the tag in place.
            .on_click(cx.listener(move |this, ev: &gpui::ClickEvent, window, cx| {
                if ev.click_count() >= 2 {
                    this.start_rename(id.clone(), window, cx);
                }
            }));
        div()
            .flex()
            .flex_col()
            .child(row)
            .when(self.colors_for.as_deref() == Some(t.id.as_str()), |d| d.child(self.render_colors(t, p, cx)))
            .when(confirming, |d| d.child(self.render_delete_confirm(t, p, cx)))
            .children(self.row_error.as_ref().filter(|(e, _)| *e == t.id).map(|(_, msg)| {
                let sel = format!("tag-error-{}", t.id);
                div().debug_selector(move || sel.clone()).px_3().pb_2().pl(px(38.)).text_xs().text_color(p.danger).child(msg.clone())
            }))
    }

    /// The colours under a tag's row: a click saves and closes; the eyedropper saves when closed.
    fn render_colors(&self, t: &Tag, p: Palette, cx: &mut Context<Self>) -> Div {
        let custom = !COLORS.iter().any(|c| c.eq_ignore_ascii_case(&t.color));
        let sel = format!("tag-colors-{}", t.id);
        div()
            .debug_selector(move || sel.clone())
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .pl(px(38.))
            .pr_3()
            .pt_1()
            .pb_3()
            .children(COLORS.iter().enumerate().map(|(i, c)| {
                let (sid, color, tid) = (format!("tag-colors-{}-{i}", t.id), c.to_string(), t.id.clone());
                let sid2 = sid.clone();
                div()
                    .id(SharedString::from(sid))
                    .debug_selector(move || sid2.clone())
                    .size(px(22.))
                    .rounded_full()
                    .cursor_pointer()
                    .bg(hex(c))
                    .when(t.color.eq_ignore_ascii_case(c), |d| d.border_2().border_color(p.text))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        (this.colors_for, this.picker) = (None, None);
                        this.save_color(tid.clone(), color.clone(), cx);
                    }))
            }))
            .when_some(self.picker.clone(), |d, picker| d.child(super::color_field::eyedropper(format!("tag-colors-{}-picker", t.id), &picker, custom.then(|| gpui::Hsla::from(hex(&t.color))), p, cx)))
    }

    fn render_delete_confirm(&self, t: &Tag, p: Palette, cx: &mut Context<Self>) -> Div {
        let id = t.id.clone();
        div()
            .mx_1()
            .mb_2()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(p.danger_border)
            .bg(p.danger_bg)
            .flex()
            .flex_col()
            .gap_3()
            .child(div().text_xs().text_color(p.text).child(format!("「{}」を削除しますか？記録した時間は残りますが、エントリーとプロジェクトからこのタグが外れます。", t.name)))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(form::button(p, "tag-delete-cancel", "キャンセル", false, true).on_click(cx.listener(|this, _, _, cx| {
                        this.deleting = None;
                        cx.notify();
                    })))
                    .child(form::button(p, "tag-delete-confirm", "タグを削除", true, true).on_click(cx.listener(move |this, _, _, cx| {
                        this.deleting = None;
                        let fut = api(&this.state, cx).delete_tag(id.clone());
                        this.write(id.clone(), fut, cx);
                    }))),
            )
    }
}

impl Render for TagSettings {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        if let Some(id) = self.rename_next.clone()
            && self.tags.iter().any(|t| t.id == id)
        {
            self.rename_next = None;
            self.start_rename(id, window, cx);
        }
        let tags = self.tags.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(1.))
            .max_w(px(640.))
            .children(self.error.clone().map(|e| div().mb_3().text_xs().text_color(p.danger).child(e)))
            .when(tags.is_empty(), |d| d.child(div().px_3().py_2().text_sm().text_color(p.text_faint).child("タグがありません")))
            .children(tags.iter().map(|t| self.render_row(t, p, cx)))
            .child(
                div()
                    .id("tag-add")
                    .debug_selector(|| "tag-add".into())
                    .flex()
                    .items_center()
                    .gap_2()
                    .mt_2()
                    .h(px(36.))
                    .px_3()
                    .rounded(px(6.))
                    .text_sm()
                    .text_color(p.text_muted)
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.hover))
                    .child(Icon::new(IconName::Plus).size_3p5())
                    .child("タグ")
                    .on_click(cx.listener(|this, _, _, cx| this.add_tag(cx))),
            )
            .child(div().mt_4().px_3().text_xs().font_weight(FontWeight::NORMAL).text_color(p.text_faint).child("名前はダブルクリックで変更、色は丸を押して選びます。"))
    }
}
