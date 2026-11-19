# 休暇と直行直帰 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 休暇（全日・前半・後半、祝日は自動）と直行直帰（プロジェクトに紐づく 1 日分のエントリー）を記録し、カレンダーで設定・表示し、レポートで休暇を含めるか切り替えられるようにする。

**Architecture:** 直行直帰は `TimeEntry.kind = direct` のエントリー（既存の集計にそのまま乗る）。休暇は新しい表 `Leave` に保存し、祝日の自動休暇はその場で計算する。休暇の分は track-core の `leave_minutes` が今の勤務時間設定から出し、レポートは `ReportQuery.include_leave` のときだけ「休暇」の行と草グラフに足す。

**Tech Stack:** Rust、rusqlite、GPUI（gpui-pre 0.3.12）、gpui-component 0.9.1。

**Spec:** `docs/superpowers/specs/2026-11-14-gpui-leave-and-direct-design.md`

## Global Constraints

- 実データ `~/.track` には書かない。動作確認はコピー DB `TRACK_DATA_DIR=$HOME/.track-gpui` だけ。
- iCloud Drive の実バックアップに触らない。実データの画面のスクリーンショットはコミットしない（撮影はアプリのウインドウだけ）。
- 移行ファイルは Tauri 版と共有する `database/migrations/`。追加だけで、既存の列・表は変えない。
- 画面の文言は日本語。休暇の表示名は「休暇」「前半休」「後半休」、直行直帰は「直行直帰」。
- 休憩は 60 分固定。稼働予定 = `max(0, work_end − work_start − 60)`、半休 = 稼働予定 ÷ 2（分は切り捨て）。
- 休暇と直行直帰は同じ日に両方持てない（後から選んだ方だけ残す）。
- 新しい UI は行の下に差し込まず、押した部品から出るポップオーバー（`settings/popover.rs` の `Trigger`／`confirm` と同じ作り）にする。
- 生の色コードは `ui/theme.rs` の外に書かない。角の丸みは RADIUS_SM／rounded_md／rounded_lg／rounded_full だけ。

## Review Focus

1. 勤務日の祝日に「なし」→ `none` が保存され自動休暇が消える。その日に直行直帰を作っても休暇は 0 のまま（Task 2・3 で固定）。
2. 休暇のある日に直行直帰を作る → 休暇の行が消え、その逆も同じ。二重に数えない（Task 3 で固定）。
3. 直行直帰はカレンダーの時間の枠に出ず、日の合計・下の帯の合計・レポートのプロジェクトの行には入る（Task 5 で固定）。
4. 「休暇を含める」オンでも、絞り込みが効いていれば休暇の行も草グラフへの加算もない（Task 4 で固定）。
5. 古い DB（`kind` 列も `Leave` 表もない）を開く・古いバックアップを復元する → 移行が流れ、既存のエントリーは `normal` になる（Task 1 で固定）。

---

### Task 1: 移行・型・祝日の移動

**Files:**
- Create: `database/migrations/0017_leave_and_direct.sql`
- Modify: `gpui/crates/track-core/src/migrate.rs`（埋め込み一覧に追加）、`database/schema.prisma`
- Modify: `gpui/crates/track-core/src/model.rs`、`gpui/crates/track-core/src/repo/entries.rs`
- Create: `gpui/crates/track-core/src/holidays.rs`（`track-app/src/core/holidays.rs` を移す）。`track-app/src/core/holidays.rs` は `pub use track_core::holidays::*;` だけにする
- Test: `gpui/crates/track-core/src/repo/repo_tests.rs`

**Interfaces:**
- Produces: `pub enum EntryKind { Normal, Direct }`（`#[default] Normal`、DB 文字列 `normal`／`direct`）、`TimeEntry.kind: EntryKind`（`#[serde(default)]`）、`pub enum LeaveKind { Full, Am, Pm, None }`（DB 文字列 `full`／`am`／`pm`／`none`）、`pub struct Leave { pub date: NaiveDate, pub kind: LeaveKind }`、`track_core::holidays::holiday_name(NaiveDate) -> Option<&'static str>`

- [ ] **Step 1: 失敗するテストを書く** — `entries_are_normal_unless_marked`: 作ったエントリーの `kind == EntryKind::Normal`。`kind_column_survives_a_raw_direct_row`: `UPDATE "TimeEntry" SET kind='direct'` 後に `db.entry(id).kind == EntryKind::Direct`。`leave_table_exists`: `SELECT COUNT(*) FROM "Leave"` が 0。`holidays_moved_to_core`: `track_core::holidays::holiday_name(2026-11-03) == Some("文化の日")`。
- [ ] **Step 2: 失敗を確認** — Run: `cargo test -p track-core entries_are_normal kind_column leave_table holidays_moved` / Expected: コンパイルエラー（型や関数がない）。
- [ ] **Step 3: 実装** — 移行 SQL: `ALTER TABLE "TimeEntry" ADD COLUMN "kind" TEXT NOT NULL DEFAULT 'normal';`、`CREATE TABLE "Leave" ("id" TEXT NOT NULL PRIMARY KEY, "date" TEXT NOT NULL, "kind" TEXT NOT NULL, "createdAt" DATETIME NOT NULL, "updatedAt" DATETIME NOT NULL);`、`CREATE UNIQUE INDEX "Leave_date_key" ON "Leave"("date");`。`ENTRY_SELECT` と `entry_from_row` に `e.kind` を足す。`TimeEntry` を組み立てているテストと `track-app/src/testing.rs`・`core/layout.rs` のテストに `kind: EntryKind::Normal` を足す。schema.prisma に `kind String @default("normal")` と `model Leave` を足す。
- [ ] **Step 4: 通ることを確認** — Run: `cargo test --workspace` / Expected: 全部 PASS。
- [ ] **Step 5: Commit** — `feat(core): 休暇の表とエントリーの種類、祝日を core へ`

### Task 2: 休暇の保存と分の計算

**Files:**
- Create: `gpui/crates/track-core/src/repo/leave.rs`（`repo/mod.rs` に追加）
- Test: `gpui/crates/track-core/src/repo/leave_tests.rs`

**Interfaces:**
- Consumes: Task 1 の型、`Db::settings()`、`holiday_name`
- Produces: `pub fn scheduled_minutes(s: &Settings) -> i64`、`Db::leaves(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<Leave>>`（`[from, to)`、行のあるものだけ）、`Db::set_leave(&mut self, date: NaiveDate, kind: Option<LeaveKind>) -> Result<()>`、`Db::leave_minutes(&self, from: NaiveDate, to: NaiveDate) -> Result<Vec<(NaiveDate, i64)>>`（毎日を順に、0 も含む）

- [ ] **Step 1: 失敗するテストを書く**
  - `scheduled_is_work_hours_less_an_hour`: 600〜1110 → 450。
  - `full_and_half_days`: `set_leave(d, Some(Full))` → 450、`Am`／`Pm` → 225。
  - `workday_holidays_are_full_leave`: 勤務日の祝日（2026-11-03 火）は行なしで 450、土曜の祝日は 0。
  - `none_cancels_a_holiday`: 祝日に `set_leave(d, None)` → 行の kind は `none`、分は 0。祝日でない日に `set_leave(d, None)` → 行が消える。
  - `manual_leave_counts_on_any_day`: 日曜に `Full` → 450。
  - `leave_follows_the_current_schedule`: 設定を 540〜1080 に変えると全日 480。
- [ ] **Step 2: 失敗を確認** — Run: `cargo test -p track-core leave` / Expected: コンパイルエラー。
- [ ] **Step 3: 実装** — `set_leave` は `Some(k)` なら upsert（`date` で一意）、`None` なら「祝日かつ勤務日」なら `none` を upsert、それ以外は削除。直行直帰との排他は Task 3 で足す。日付は `NaiveDate::to_string()`（`YYYY-MM-DD`）で保存。
- [ ] **Step 4: 通ることを確認** — Run: `cargo test -p track-core` / Expected: PASS。
- [ ] **Step 5: Commit** — `feat(core): 休暇の保存と分の計算`

### Task 3: 直行直帰と排他

**Files:**
- Modify: `gpui/crates/track-core/src/repo/leave.rs`（直行直帰も同じファイルに置く）
- Test: `gpui/crates/track-core/src/repo/leave_tests.rs`

**Interfaces:**
- Consumes: Task 2、`Db::create_entry`、`Db::delete_entry`
- Produces: `Db::direct_entry(&self, date: NaiveDate) -> Result<Option<TimeEntry>>`、`Db::set_direct(&mut self, date: NaiveDate, project_id: &str) -> Result<TimeEntry>`、`Db::remove_direct(&mut self, date: NaiveDate) -> Result<()>`

- [ ] **Step 1: 失敗するテストを書く**
  - `direct_is_one_day_of_work`: `set_direct(d, p)` → `start` = d の勤務開始、`end` = 勤務終了、`break_minutes` = 60、`kind` = Direct、タイトル「直行直帰」、正味 450 分。
  - `direct_replaces_the_days_direct`: 同じ日に別プロジェクトで作り直すと 1 件だけ、プロジェクトが新しい方。
  - `direct_clears_leave` と `leave_clears_direct`: 全日休暇の日に直行直帰 → 休暇の行がなく分は 0。直行直帰の日に `set_leave(d, Some(Am))` → 直行直帰が消える。
  - `direct_on_a_holiday_cancels_the_auto_leave`: 勤務日の祝日に直行直帰 → 休暇の分は 0（`none` が保存される）。
  - `remove_direct_deletes_it`。
- [ ] **Step 2: 失敗を確認** — Run: `cargo test -p track-core direct` / Expected: コンパイルエラー。
- [ ] **Step 3: 実装** — 1 つのトランザクション（savepoint）で、その日の `kind='direct'` を消し、勤務時間でエントリーを作って `kind='direct'` にし、その日の休暇を「なし」にする（祝日なら `none`）。`set_leave(Some(_))` はその日の直行直帰を消す。
- [ ] **Step 4: 通ることを確認** — Run: `cargo test -p track-core` / Expected: PASS。
- [ ] **Step 5: Commit** — `feat(core): 直行直帰と休暇の排他`

### Task 4: レポートの休暇

**Files:**
- Modify: `gpui/crates/track-core/src/model.rs`（`ReportQuery.include_leave: bool`、`#[serde(default)]`）、`gpui/crates/track-core/src/reports.rs`
- Modify: `ReportQuery` を組み立てている箇所（`reports_tests.rs` の `q`、`track-app/src/reports/mod.rs` の `current_query`・`child_query` など。既定は `false`）
- Test: `gpui/crates/track-core/src/reports_tests.rs`

**Interfaces:**
- Consumes: Task 2 の `leave_minutes`
- Produces: 休暇の行のキー `pub const LEAVE_KEY: &str = "__leave__"`、表示名「休暇」、色は既存の NONE_COLOR と別の灰系（`#a3a3a3` を reports.rs の定数に）

- [ ] **Step 1: 失敗するテストを書く**
  - `leave_row_only_when_asked`: 週に全日休暇 1 日 → `include_leave=false` では行なし・合計は作業だけ、`true` では `__leave__` の行 450 分が入り合計にも足される。
  - `no_leave_row_with_filters`: `include_leave=true` かつ `project_ids` あり → 行なし。
  - `weekly_copy_never_has_leave`: `include_leave=true` でも `weekly_copy` の文字列に「休暇」が出ない。
  - `heatmap_adds_leave_when_asked`: 休暇の日の分が `include_leave` のときだけ 450 増える。絞り込みありでは増えない。
- [ ] **Step 2: 失敗を確認** — Run: `cargo test -p track-core leave_row no_leave_row heatmap_adds_leave` / Expected: コンパイルエラー。
- [ ] **Step 3: 実装** — `report()` と `heatmap()` の最後で、`include_leave && 絞り込みなし` のとき `leave_minutes` を範囲の日付で足す（`report` は行を足して並べ直す）。`report_entries` は `__leave__` を空で返す。`weekly_copy` は `include_leave: false` に固定して呼ぶ（週の合計のコピーに休暇を入れない）。
- [ ] **Step 4: 通ることを確認** — Run: `cargo test --workspace` / Expected: PASS。
- [ ] **Step 5: Commit** — `feat(core): レポートに休暇を含める`

### Task 5: カレンダー（設定のポップオーバー・表示）

**Files:**
- Modify: `gpui/crates/track-app/src/data/mod.rs`（`leaves`・`set_leave`・`set_direct`・`remove_direct`・`leave_minutes` を足す）
- Modify: `gpui/crates/track-app/src/calendar/mod.rs`
- Create: `gpui/crates/track-app/src/calendar/day_menu.rs`（見出しのボタンとポップオーバー）
- Test: `gpui/crates/track-app/src/calendar_tests.rs`

**Interfaces:**
- Consumes: Task 2・3 の Db 関数
- Produces: `CalendarView.leaves: HashMap<NaiveDate, LeaveKind>`（`reload` でエントリーと一緒に読む）。デバッグ id: `day-menu-{YYYY-MM-DD}`（ボタン）、`day-leave-full`・`day-leave-am`・`day-leave-pm`・`day-leave-none`、`day-direct-{project_id}`、`day-direct-remove`、`day-mark-{YYYY-MM-DD}`（見出しの印）

- [ ] **Step 1: 失敗するテストを書く**
  - `choosing_full_leave_marks_the_day`: `day-menu-2026-11-12` → `day-leave-full` → DB に Full、見出しに `day-mark-2026-11-12`。
  - `direct_is_not_in_the_time_grid_but_counts`: `day-direct-p1` → `entry-{id}` が描かれない、日の合計（`visible_total_minutes`）が 450 増える、印が出る。
  - `holiday_none_option_is_labelled_for_holidays`: 祝日の日のメニューで `day-leave-none` が出て、押すと休暇の分 0。
- [ ] **Step 2: 失敗を確認** — Run: `cargo test -p track-app choosing_full_leave direct_is_not_in holiday_none` / Expected: FAIL（id がない）。
- [ ] **Step 3: 実装** — 見出しの日付の右（合計の左）に `controls::icon_button`（アイコン Calendar 系、ツールチップ「休暇・直行直帰」）を `popover::Trigger` で包み、ポップオーバーに「休暇」の 4 つ（祝日なら「なし」を「休暇にしない」）と「直行直帰」のプロジェクト一覧（アーカイブ以外、色の丸付き）と、ある日だけ「直行直帰を外す」。選ぶと保存して閉じ、`state.refresh_all`。`render_column` は `kind == Direct` を時間の枠から除く。印は休暇なら「休暇／前半休／後半休」、直行直帰なら「直行直帰 プロジェクト名」（プロジェクトの色を薄く）。祝日の自動休暇は祝日名の印のままで、ツールチップに「休暇（祝日）」と出す。
- [ ] **Step 4: 通ることを確認** — Run: `cargo test --workspace` / Expected: PASS。
- [ ] **Step 5: Commit** — `feat(app): カレンダーで休暇と直行直帰を設定する`

### Task 6: レポートの「休暇を含める」

**Files:**
- Modify: `gpui/crates/track-app/src/persist.rs`（`include_leave: bool`）、`gpui/crates/track-app/src/reports/mod.rs`
- Test: `gpui/crates/track-app/src/reports_tests.rs`

**Interfaces:**
- Consumes: Task 4 の `ReportQuery.include_leave`・`LEAVE_KEY`
- Produces: ツールバーのチップ `include-leave`（`controls::chip`）

- [ ] **Step 1: 失敗するテストを書く** — `include_leave_toggle_adds_the_leave_row`: 今週に全日休暇を入れ、`include-leave` を押す → `persist.include_leave == true`、`data.rows` に `__leave__`、もう一度押すと消える。`leave_row_does_not_expand`: `row-__leave__` を押しても子の読み込みが始まらない。
- [ ] **Step 2: 失敗を確認** — Run: `cargo test -p track-app include_leave leave_row_does_not` / Expected: FAIL。
- [ ] **Step 3: 実装** — `current_query` に `include_leave` を入れる。ツールバーの右側（表示の切替のあと）にチップ「休暇を含める」。`__leave__` の行は開くボタンを出さない。
- [ ] **Step 4: 通ることを確認** — Run: `cargo test --workspace` と `cargo clippy --workspace --all-targets -- -D warnings` / Expected: PASS・警告なし。
- [ ] **Step 5: Commit** — `feat(app): レポートの休暇を含める切り替え`

### Task 7: 文書と実機の確認

**Files:**
- Modify: `gpui/README.md`、`gpui/NOTES.md`

- [ ] **Step 1:** README のカレンダーとレポートの節に、休暇・直行直帰・切り替えを書く。NOTES に計算のきまり（休憩 60 分固定、今の設定で計算、排他）を書く。
- [ ] **Step 2:** コピー DB で起動し、カレンダーの見出しのメニューを開いて撮影（コミットしない）。休暇を 1 日付けてレポートで切り替えを確認し、付けた休暇は消して元に戻す。
- [ ] **Step 3: 全テスト** — Run: `cargo test --workspace` / Expected: PASS。
- [ ] **Step 4: Commit** — `docs(gpui): 休暇と直行直帰`
