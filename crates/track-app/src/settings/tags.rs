//! 「タグ」: one row per tag, edited in place. Double-click a row to rename it (Enter saves; Esc,
//! leaving the field or clicking elsewhere cancels), press its colour dot for a popover of colours,
//! and the trash icon (shown on hover) deletes it after a confirmation popover. Each row says how
//! many projects and entries use the tag (or why a save failed). Everything saves at once; nothing
//! opens under a row.
use super::popover::{self, Confirm, Trigger};
use crate::app::AppState;
use crate::data::{DataClient, DataResult};
use crate::ui::theme::*;
use gpui::{
    AppContext, Context, Div, Entity, InteractiveElement, IntoElement, ParentElement, Render, SharedString, StatefulInteractiveElement, Styled,
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
    pub picker: Option<Entity<ColorPickerState>>,
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

    /// Opens the colour popover for a tag, with sliders set to its colour.
    fn open_colors(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.save_pending_color(cx);
        let Some(color) = self.tags.iter().find(|t| t.id == id).map(|t| t.color.clone()) else { return };
        let picker = cx.new(|cx| ColorPickerState::new(window, cx).default_value(hex(&color)));
        // Put the colour into the sliders and the hex field (the eyedropper does this when it opens).
        picker.update(cx, |s, cx| s.sync_pending_value(window, cx));
        let tid = id.clone();
        // Slider steps are kept and saved once, when the popover closes.
        cx.subscribe(&picker, move |this, _, ev: &ColorPickerEvent, _| {
            if let ColorPickerEvent::Change(Some(c)) = ev {
                this.pending_color = Some((tid.clone(), *c));
            }
        })
        .detach();
        self.cancel_rename(cx);
        (self.colors_for, self.picker) = (Some(id), Some(picker));
        cx.notify();
    }

    fn close_colors(&mut self, cx: &mut Context<Self>) {
        self.save_pending_color(cx);
        (self.colors_for, self.picker) = (None, None);
        cx.notify();
    }

    fn render_row(&self, t: &Tag, p: Palette, cx: &mut Context<Self>) -> gpui::Stateful<Div> {
        let group = SharedString::from(format!("tag-row-group-{}", t.id));
        let id = t.id.clone();
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
        let usage_sel = format!("tag-usage-{}", t.id);
        let confirming = self.deleting.as_deref() == Some(t.id.as_str());
        div()
            .id(SharedString::from(row_sel))
            .debug_selector(move || row_sel2.clone())
            .group(group.clone())
            .flex()
            .items_center()
            .gap_3()
            .h(px(40.))
            .px_3()
            .rounded_md()
            .text_sm()
            .hover(move |s| s.bg(p.hover))
            .child(self.render_dot(t, p, cx))
            .child(name)
            .child(match self.row_error.as_ref().filter(|(e, _)| *e == t.id) {
                // A failed save shows in the row, where the usage is.
                Some((_, msg)) => {
                    let sel = format!("tag-error-{}", t.id);
                    div().debug_selector(move || sel.clone()).flex_none().text_xs().text_color(p.danger).child(msg.clone())
                }
                None => div().debug_selector(move || usage_sel.clone()).flex_none().text_xs().text_color(p.text_faint).child(self.usage_label(&t.id)),
            })
            .child(self.render_delete(t, confirming, group, p, cx))
            // Double-click renames the tag in place.
            .on_click(cx.listener(move |this, ev: &gpui::ClickEvent, window, cx| {
                if ev.click_count() >= 2 {
                    this.start_rename(id.clone(), window, cx);
                }
            }))
    }

    /// The colour dot, opening a popover of the preset colours (a click saves) and sliders (saved
    /// when the popover closes).
    fn render_dot(&self, t: &Tag, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.colors_for.as_deref() == Some(t.id.as_str());
        let dot_sel = format!("tag-color-{}", t.id);
        let dot = div()
            .debug_selector(move || dot_sel.clone())
            .size(px(14.))
            .flex_none()
            .rounded_full()
            .cursor_pointer()
            .bg(hex(&t.color))
            .when(open, |d| d.border_2().border_color(p.text));
        let this = cx.entity().downgrade();
        let id = t.id.clone();
        gpui_component::popover::Popover::new(SharedString::from(format!("tag-color-pop-{}", t.id)))
            .open(open)
            .on_open_change(move |open: &bool, window, cx| {
                let (open, id) = (*open, id.clone());
                let _ = this.update(cx, |this, cx| if open { this.open_colors(id, window, cx) } else { this.close_colors(cx) });
            })
            .trigger(Trigger::new(dot))
            .when(open, |pop| {
                let sel = format!("tag-colors-{}", t.id);
                pop.w(px(264.)).child(
                    div()
                        .debug_selector(move || sel.clone())
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_1()
                        .child(div().flex().flex_wrap().gap_2().children(COLORS.iter().enumerate().map(|(i, c)| {
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
                                    (this.colors_for, this.picker, this.pending_color) = (None, None, None);
                                    this.save_color(tid.clone(), color.clone(), cx);
                                }))
                        })))
                        .children(self.picker.as_ref().map(|picker| super::color_field::picker_controls(picker, p, cx))),
                )
            })
    }

    /// The trash icon (shown while the row is hovered), confirming in a popover.
    fn render_delete(&self, t: &Tag, confirming: bool, group: SharedString, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let sel = format!("tag-delete-{}", t.id);
        let icon = crate::ui::controls::icon_button(&p, SharedString::from(format!("tag-delete-icon-{}", t.id)), crate::ui::icons::trash(), "削除する")
            .debug_selector(move || sel.clone())
            .opacity(if confirming { 1. } else { 0. })
            .group_hover(group, |s| s.opacity(1.));
        let (this, this2, id, id2) = (cx.entity().downgrade(), cx.entity().downgrade(), t.id.clone(), t.id.clone());
        popover::confirm(
            SharedString::from(format!("tag-delete-pop-{}", t.id)),
            icon,
            confirming,
            move |open, _, cx| {
                let id = id.clone();
                let _ = this.update(cx, |this, cx| {
                    if open {
                        this.cancel_rename(cx);
                    }
                    this.deleting = open.then_some(id);
                    cx.notify();
                });
            },
            Confirm {
                message: format!("「{}」を削除しますか？記録した時間は残りますが、エントリーとプロジェクトからこのタグが外れます。", t.name),
                cancel_id: "tag-delete-cancel",
                confirm_id: "tag-delete-confirm",
                confirm_label: "タグを削除",
            },
            move |_, cx| {
                let id = id2.clone();
                let _ = this2.update(cx, |this, cx| {
                    this.deleting = None;
                    let fut = api(&this.state, cx).delete_tag(id.clone());
                    this.write(id, fut, cx);
                });
            },
            p,
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
                    .rounded_md()
                    .text_sm()
                    .text_color(p.text_muted)
                    .cursor_pointer()
                    .hover(move |s| s.bg(p.hover))
                    .child(Icon::new(IconName::Plus).size_3p5())
                    .child("タグ")
                    .on_click(cx.listener(|this, _, _, cx| this.add_tag(cx))),
            )
    }
}
