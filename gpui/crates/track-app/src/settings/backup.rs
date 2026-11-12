//! 「バックアップと復元」 (SettingsPage「保存の設定」「復元ポイント」). Settings go to config.json;
//! snapshots and restore run on the DB thread.
use super::form::{self, Actions};
use crate::app::AppState;
use crate::ui::theme::*;
use chrono::Local;
use gpui::{
    AppContext, Context, Entity, FontWeight, InteractiveElement, IntoElement, ParentElement, PathPromptOptions, Render, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px,
};
use gpui_component::input::{Input, InputState};
use track_core::backup::SnapshotInfo;
use track_core::config::{Config, ConfigPatch, export_dir_suggestions};

pub struct BackupSettings {
    state: Entity<AppState>,
    pub export_dir: Entity<InputState>,
    pub interval: Entity<InputState>,
    pub keep: Entity<InputState>,
    saved: Config,
    pub snapshots: Vec<SnapshotInfo>,
    restoring: Option<SnapshotInfo>,
    busy: bool,
    saving: bool,
    save_error: Option<String>,
    notice: Option<String>,
}

impl BackupSettings {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let saved = state.read(cx).api.full_config();
        let input = |v: String, window: &mut Window, cx: &mut Context<Self>| {
            let e = cx.new(|cx| InputState::new(window, cx).default_value(v));
            cx.observe(&e, |_, _, cx| cx.notify()).detach();
            e
        };
        let export_dir = input(saved.export_dir.to_string_lossy().into_owned(), window, cx);
        let interval = input(saved.backup_interval_hours.to_string(), window, cx);
        let keep = input(saved.backup_keep.to_string(), window, cx);
        let mut s = BackupSettings {
            state,
            export_dir,
            interval,
            keep,
            saved,
            snapshots: vec![],
            restoring: None,
            busy: false,
            saving: false,
            save_error: None,
            notice: None,
        };
        s.reload(cx);
        s
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        self.snapshots = self.state.read(cx).api.snapshots();
        cx.notify();
    }

    fn numbers(&self, cx: &gpui::App) -> (Option<i64>, Option<i64>) {
        (self.interval.read(cx).value().trim().parse().ok(), self.keep.read(cx).value().trim().parse().ok())
    }

    fn draft_dir(&self, cx: &gpui::App) -> String {
        self.export_dir.read(cx).value().trim().to_string()
    }

    fn dirty(&self, cx: &gpui::App) -> bool {
        let (i, k) = self.numbers(cx);
        std::path::Path::new(&self.draft_dir(cx)) != self.saved.export_dir || i != Some(self.saved.backup_interval_hours) || k != Some(self.saved.backup_keep)
    }

    fn validation(&self, cx: &gpui::App) -> Option<&'static str> {
        let (i, k) = self.numbers(cx);
        if self.draft_dir(cx).is_empty() {
            Some("保存先フォルダを指定してください")
        } else if !i.is_some_and(|i| (0..=720).contains(&i)) {
            Some("実行間隔は0〜720時間で指定してください")
        } else if !k.is_some_and(|k| (1..=1000).contains(&k)) {
            Some("保存数は1〜1000件で指定してください")
        } else {
            None
        }
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let s = self.saved.clone();
        self.export_dir.update(cx, |i, cx| i.set_value(s.export_dir.to_string_lossy().into_owned(), window, cx));
        self.interval.update(cx, |i, cx| i.set_value(s.backup_interval_hours.to_string(), window, cx));
        self.keep.update(cx, |i, cx| i.set_value(s.backup_keep.to_string(), window, cx));
        self.save_error = None;
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        if self.validation(cx).is_some() || self.saving {
            return;
        }
        let (i, k) = self.numbers(cx);
        let dir = self.draft_dir(cx);
        let patch = ConfigPatch {
            export_dir: (std::path::Path::new(&dir) != self.saved.export_dir).then_some(dir),
            backup_interval_hours: i,
            backup_keep: k,
            ..Default::default()
        };
        self.saving = true;
        let fut = self.state.read(cx).api.save_config(patch);
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                this.saving = false;
                match result {
                    Ok(c) => {
                        this.saved = c;
                        this.save_error = None;
                        this.reload(cx);
                        this.state.update(cx, |s, cx| s.refresh_all(cx));
                    }
                    Err(e) => {
                        this.save_error = Some(match e.code.as_str() {
                            "not_writable" => "保存先フォルダに書き込めません".into(),
                            "not_absolute" => "保存先は絶対パスで指定してください".into(),
                            _ => e.message,
                        })
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn pick_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("選択".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(p) = paths.into_iter().next()
            {
                let _ = this.update_in(cx, |this, window, cx| {
                    this.export_dir.update(cx, |i, cx| i.set_value(p.to_string_lossy().into_owned(), window, cx));
                });
            }
        })
        .detach();
    }

    fn backup_now(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        let fut = self.state.read(cx).api.backup_now();
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.notice = Some(match result {
                    Ok(_) => "バックアップを取りました".into(),
                    Err(e) => format!("バックアップに失敗しました: {}", e.message),
                });
                this.reload(cx);
            });
        })
        .detach();
        cx.notify();
    }

    fn restore(&mut self, snap: SnapshotInfo, cx: &mut Context<Self>) {
        self.restoring = None;
        self.busy = true;
        let fut = self.state.read(cx).api.restore(snap.path);
        cx.spawn(async move |this, cx| {
            let result = fut.await;
            let _ = this.update(cx, |this, cx| {
                this.busy = false;
                this.notice = Some(match result {
                    Ok(_) => "リストアしました".into(),
                    Err(e) if e.code == "invalid_snapshot" => "このファイルはリストアできません".into(),
                    Err(e) => format!("リストアに失敗しました: {}", e.message),
                });
                this.reload(cx);
                this.state.update(cx, |s, cx| s.refresh_all(cx));
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for BackupSettings {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dirty = self.dirty(cx);
        let validation = self.validation(cx);
        let error = validation.map(str::to_string).or(self.save_error.clone());
        let can_save = dirty && validation.is_none() && !self.saving;
        let home = dirs::home_dir().unwrap_or_default();
        // The dev build hides iCloud Drive: the real app's backups live there.
        let suggestions = export_dir_suggestions(&home, crate::ui::integrations_dialog::DEV_NAMES);
        let default_dir = Config::defaults(&self.state.read(cx).api.data_dir).export_dir;
        let number = |input: &Entity<InputState>, unit: &'static str| div().flex().items_center().gap_2().child(div().w(px(90.)).child(Input::new(input))).child(div().text_sm().text_color(n600()).child(unit));
        let rows = self.snapshots.iter().enumerate().map(|(i, s)| {
            let id = format!("backup-row-{i}");
            let sel = id.clone();
            let snap = s.clone();
            div()
                .id(SharedString::from(id))
                .debug_selector(move || sel.clone())
                .flex()
                .items_center()
                .gap_3()
                .px_5()
                .py_3()
                .border_b_1()
                .border_color(n100())
                .child(div().text_sm().font_weight(FontWeight::MEDIUM).child(s.created_at.with_timezone(&Local).format("%Y/%-m/%-d %-H:%M:%S").to_string()))
                .when(i == 0, |d| d.child(div().debug_selector(|| "backup-latest".into()).px_2().rounded_md().bg(gpui::rgb(0xecfdf5)).text_xs().text_color(gpui::rgb(0x065f46)).child("最新")))
                .child(div().text_xs().text_color(n500()).child(if s.auto { "自動バックアップ" } else { "手動バックアップ" }))
                .child(div().flex_1())
                .child(div().text_xs().text_color(n500()).child(format!("{} KB", s.bytes / 1024)))
                .child(form::button(format!("backup-restore-{i}"), "復元", false, !self.busy).on_click(cx.listener(move |this, _, _, cx| {
                    this.restoring = Some(snap.clone());
                    cx.notify();
                })))
        });
        div()
            .child(
                form::card("保存の設定", "iCloud Driveなど、普段同期されるフォルダも保存先にできます。")
                    .child(form::row(
                        "保存先フォルダ",
                        "Track本体ではなく、作成されるバックアップだけを同期してください。",
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(div().flex_1().child(Input::new(&self.export_dir)))
                                    .child(form::button("backup-pick", "選択", false, true).on_click(cx.listener(|this, _, w, cx| this.pick_folder(w, cx)))),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .children(suggestions.into_iter().map(|(label, path)| {
                                        form::button(SharedString::from(format!("backup-suggest-{label}")), &format!("{label}を使う"), false, true).on_click(cx.listener(move |this, _, w, cx| {
                                            this.export_dir.update(cx, |i, cx| i.set_value(path.to_string_lossy().into_owned(), w, cx));
                                        }))
                                    }))
                                    .child(form::button("backup-default", "既定に戻す", false, true).on_click(cx.listener(move |this, _, w, cx| {
                                        this.export_dir.update(cx, |i, cx| i.set_value(default_dir.to_string_lossy().into_owned(), w, cx));
                                    }))),
                            ),
                    ))
                    .child(form::row("実行間隔", "0時間にすると自動バックアップを停止します。", number(&self.interval, "時間")))
                    .child(form::row("保存数", "手動で作成したバックアップは削除されません。", number(&self.keep, "件"))),
            )
            .child(form::actions(
                Actions { prefix: "backup", dirty, saving: self.saving, error: error.as_deref() },
                form::button("backup-discard", "変更を破棄", false, !self.saving).on_click(cx.listener(|this, _, w, cx| this.reset(w, cx))),
                form::button("backup-save", if self.saving { "保存中…" } else { "変更を保存" }, true, can_save).on_click(cx.listener(|this, _, _, cx| this.save(cx))),
            ))
            .when_some(self.restoring.clone(), |d, snap| {
                d.child(
                    div()
                        .mb_5()
                        .p_4()
                        .rounded_lg()
                        .border_1()
                        .border_color(gpui::rgb(0xfde68a))
                        .bg(gpui::rgb(0xfffbeb))
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("この時点へ復元しますか？"))
                        .child(div().text_xs().text_color(n700()).child(format!(
                            "{} の状態に戻します。現在のデータをこの時点の内容で置き換えます。実行直前の状態も自動でバックアップされます。",
                            snap.created_at.with_timezone(&Local).format("%Y/%-m/%-d %-H:%M:%S")
                        )))
                        .child(
                            div()
                                .flex()
                                .justify_end()
                                .gap_2()
                                .child(form::button("backup-restore-cancel", "キャンセル", false, true).on_click(cx.listener(|this, _, _, cx| {
                                    this.restoring = None;
                                    cx.notify();
                                })))
                                .child(form::button("backup-restore-confirm", "この時点へ復元", true, true).on_click(cx.listener(move |this, _, _, cx| this.restore(snap.clone(), cx)))),
                        ),
                )
            })
            .child(
                form::card("復元ポイント", "過去の状態を選ぶと、現在の状態を退避してから復元します。")
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .px_5()
                            .py_3()
                            .border_b_1()
                            .border_color(n100())
                            .child(form::button("backup-now", if self.busy { "処理中…" } else { "今すぐバックアップ" }, true, !self.busy).on_click(cx.listener(|this, _, _, cx| this.backup_now(cx))))
                            .when_some(self.notice.clone(), |d, n| d.child(div().text_xs().text_color(n600()).child(n))),
                    )
                    .when(self.snapshots.is_empty(), |d| {
                        d.child(
                            div()
                                .px_5()
                                .py_4()
                                .child(div().text_sm().text_color(n600()).child("バックアップはまだありません"))
                                .child(div().text_xs().text_color(n500()).child("「今すぐバックアップ」から最初の復元ポイントを作成できます。")),
                        )
                    })
                    .children(rows),
            )
    }
}
