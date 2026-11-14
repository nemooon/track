# GPUI 版 見た目のブラッシュアップ（Zed 風） Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 意味の名前で色を引く層（明るい配色）を作って全画面を置き換え、明るいヘッダー・共通の状態バー・淡い色の工数ブロック・割合バー付きの明細表で Zed 風の見た目にする。

**Architecture:** `ui/theme.rs` に `Palette`（`Copy` な意味色の組、`gpui::Global`）を置き、`palette(cx)` で引く。起動時に明るい配色を入れ、gpui-component の Theme も明るいに固定して主要色を写す。生の色関数（`n200()` など）は最後に削除し、コンパイラで取り残しを防ぐ。

**Tech Stack:** Rust 1.98.1, GPUI (gpui-pre 0.3.12), gpui-component 0.9.1

**Spec:** `docs/superpowers/specs/2026-11-14-gpui-zed-look-design.md`

## Global Constraints

- 明るい配色だけを作る。暗い配色の値は作らないが、`Palette` に値の組を足せば切り替わる形にする
- macOS が暗い配色でも明るく表示する（gpui-component の `ThemeMode::Light` に固定）
- 機能・操作は変えない。変えるのは見た目と、レポートの集計列の削除・割合バー・状態バーの追加だけ
- レポートの数字（合計・行の時間・割合）、明細の展開・全展開・まとめる、保存済みの展開状態を失わない
- 日付の移動は各画面のツールバーのまま（ヘッダーに集めない）
- 数字（時間・割合・時刻）は等幅フォント `MONO = "Menlo"`。単位と日本語はそのまま
- 既存の debug selector（`ai-op-banner`・`ai-op-undo`・`ai-op-close`・`ai-op-error`、カレンダー・レポート・設定の各 ID）は残す
- 動作確認は今までどおりコピー DB（`~/.track-gpui`）とアプリのウインドウだけの撮影。撮影は commit しない

## Review Focus

1. 古い保存ファイル（`summaryCollapsed: true` を含む）を読んでも起動し、レポートが全幅で出る（Task 5 で固定）
2. 割合バーの幅が期間合計に対する割合と一致し、合計 0 分や 1 行だけでも崩れない（Task 5 で固定）
3. AI の通知が状態バーへ移っても、まとめて戻す・止まったら理由・×で区切る、が今までどおり動く（Task 3 で既存テストを流用）
4. 状態バーの「表示中の合計」が、表示範囲外の工数を数えない（日をまたいで表示範囲の端にかかる工数も含めない）（Task 4 で固定）
5. 設定ウインドウ（別ウインドウ）も同じ配色を引く（Task 1 で固定）

---

### Task 1: 意味色の層と明るい配色

**Files:**
- Modify: `gpui/crates/track-app/src/ui/theme.rs`
- Modify: `gpui/crates/track-app/src/main.rs`、`gpui/crates/track-app/src/testing.rs`（初期化）
- Test: `gpui/crates/track-app/src/ui/theme.rs`（`mod tests`）、`gpui/crates/track-app/src/settings_tests.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub bg: Hsla,            // 本文の背景（白）
    pub surface: Hsla,       // ヘッダー・状態バー・ツールバーの面（Zed の明るいタイトルバー程度の薄いグレー）
    pub elevated: Hsla,      // ポップオーバー・ダイアログ
    pub border: Hsla,        // 主要な境界線
    pub border_subtle: Hsla, // 表の行・グリッドの細線
    pub text: Hsla,
    pub text_muted: Hsla,
    pub text_faint: Hsla,    // 時刻目盛りなど
    pub hover: Hsla,
    pub selected: Hsla,      // 選択中のボタン・タブの面
    pub accent: Hsla,        // 今日・リンク・フォーカス
    pub accent_text: Hsla,   // accent 上の文字
    pub danger: Hsla,
    pub danger_bg: Hsla,
    pub warning_bg: Hsla,
    pub now_line: Hsla,      // 現在時刻線
    pub work_hours: Hsla,    // 勤務時間の面（時間外は bg のまま網掛け）
}
impl gpui::Global for Palette {}
impl Palette { pub fn light() -> Self; }
pub const MONO: &str = "Menlo";
pub fn palette(cx: &gpui::App) -> Palette;          // 未設定なら Palette::light()
pub fn install_light_theme(cx: &mut gpui::App);    // Palette を set_global、gpui_component::Theme を ThemeMode::Light にして bg/foreground/border/muted_foreground/popover/accent などを写す
```

- [ ] **Step 1: 失敗するテストを書く**
  - `light_palette_is_readable`（theme.rs）: 相対輝度のコントラスト比で `text` 対 `bg` ≥ 7.0、`text_muted` 対 `bg` ≥ 4.5、`text` 対 `surface` ≥ 7.0。比の計算はテスト内のヘルパー（WCAG の式）
  - `light_theme_is_forced_and_shared_with_settings_window`（settings_tests）: `install_light_theme` 後、`gpui_component::Theme::global(cx).mode` が Light。設定ウインドウを開き、そのウインドウ内で読んだ `palette(cx)` がメインと同じ（`==`）
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-app theme` / Expected: FAIL（`Palette` が無い）
- [ ] **Step 3: 実装** — `main.rs` と `testing.rs` の `gpui_component::init(cx)` 直後に `install_light_theme(cx)`。既存の生の色関数はこのタスクでは残す
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test -p track-app` / Expected: PASS
- [ ] **Step 5: Commit** — `feat(app): 意味の色の層と明るい配色`

### Task 2: 明るいヘッダーと共通部品

**Files:**
- Modify: `gpui/crates/track-app/src/app.rs`（`render_header`、本体の背景）
- Modify: `gpui/crates/track-app/src/ui/controls.rs`、`ui/date_nav.rs`、`ui/integrations_dialog.rs`
- Test: `gpui/crates/track-app/src/app_tests.rs`

**Interfaces:**
- Consumes: `palette`, `Palette`, `MONO`
- Produces: `controls::toolbar(p: &Palette) -> Div`、`controls::group(p: &Palette) -> Div`、`controls::button(p: &Palette, id, active, icon_only) -> Stateful<Div>`、`controls::error_banner(p: &Palette, retry_id, message)`、`controls::disabled(p: &Palette, d, is_disabled)`（第 1 引数に `&Palette` を足す）。ヘッダーの debug selector `app-header`

ヘッダー: 背景 `surface`、下に `border`。左は信号機分の余白（いまの値のまま）＋カレンダー／レポートのタブ（選択中は `bg` の面＋`border` の細枠、それ以外は `text_muted`）、右は設定ボタンだけ。ドラッグ領域の挙動は変えない。

- [ ] **Step 1: 失敗するテストを書く** — `header_has_only_tabs_and_settings`: `app-header` の中に `tab-calendar`・`tab-reports`・`open-settings` があり、`nav-label`（日付表示）はヘッダーの bounds の外（下）にある。既存の ID が違えば実在の ID に合わせる
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-app header_has_only` / Expected: FAIL（`app-header` が無い）
- [ ] **Step 3: 実装** — 上記ファイルの生の色を `Palette` の名前に置き換える。controls の呼び出し元（calendar・reports・settings）は引数を足すだけ（色の置き換えは各タスク）
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test -p track-app` / Expected: PASS
- [ ] **Step 5: Commit** — `feat(app): 明るいヘッダーと共通部品の配色`

### Task 3: 共通の状態バー（AI の通知を移す）

**Files:**
- Create: `gpui/crates/track-app/src/ui/status_bar.rs`
- Delete: `gpui/crates/track-app/src/ui/ai_operation_banner.rs`（中身は status_bar へ）
- Modify: `gpui/crates/track-app/src/app.rs`（本体の下に状態バー、上の帯を外す）
- Test: `gpui/crates/track-app/src/app_tests.rs`

**Interfaces:**
- Consumes: `AiOpNotice`（`count()`・`summary()`・`error`・`busy`）、`palette`
- Produces:

```rust
pub fn render(left: Vec<AnyElement>, notice: Option<AiOpNotice>, cx: &mut Context<TrackApp>) -> impl IntoElement; // debug selector "status-bar"
```

高さ 24px、背景 `surface`、上に `border`、文字は小さく `text_muted`。右側に AI の通知（`ai-op-banner` を通知部分のコンテナに付ける。`ai-op-undo`・`ai-op-close`・`ai-op-error` も同じ）。要約と理由は 1 行で末尾省略。左の中身は Task 4（カレンダー）と Task 5（レポート）が渡す。このタスクでは空の Vec。

- [ ] **Step 1: 失敗するテストを書く** — `ai_notice_lives_in_the_status_bar`: AI の操作を 1 つ起こして refresh した後、`ai-op-banner` の bounds が `status-bar` の bounds の中にあり、`status-bar` の下端がウインドウ下端に接する（±1px）
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-app ai_notice_lives` / Expected: FAIL
- [ ] **Step 3: 実装**
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test -p track-app` / Expected: PASS（既存の banner 系テスト 7 件も通ること）
- [ ] **Step 5: Commit** — `feat(app): 共通の状態バーに AI の通知を移す`

### Task 4: カレンダー（淡い色の面・状態バーの中身）

**Files:**
- Modify: `gpui/crates/track-app/src/calendar/mod.rs`、`calendar/entry_block.rs`、`calendar/grid.rs`、`calendar/picker.rs`、`calendar/edit_dialog.rs`
- Modify: `gpui/crates/track-app/src/app.rs`（カレンダー表示時に状態バーの左へ渡す）
- Test: `gpui/crates/track-app/src/calendar_tests.rs`

**Interfaces:**
- Consumes: `palette`, `MONO`, `status_bar::render`
- Produces:

```rust
impl CalendarView {
    /// 表示中の日（`days(window, cx)`）に入る工数の合計分。日をまたいで端にかかる工数は、その日に入る分だけ数える
    pub fn visible_total_minutes(&self, days: &[NaiveDate]) -> i64;
    /// 選択中の工数「<タイトル> HH:MM–HH:MM（H:MM）」。未選択なら None。タイトルが無ければ「（タイトルなし）」
    pub fn selection_label(&self) -> Option<String>;
}
```

工数ブロック: 面はプロジェクト色を約 12% で `bg` に混ぜた色、枠は約 30%、文字は `text`、プロジェクト名は `text_muted`。選択中・ホバー中は枠を濃くする（いまの影の強弱は残してよい）。時刻・日の合計は `MONO`。色の網掛け・現在時刻線・ホバー線・ホバー時刻ラベルは `Palette` の名前へ。状態バーの左: 「合計 <MONO の時間>」と `selection_label`（あるとき）。

- [ ] **Step 1: 失敗するテストを書く**
  - `visible_total_counts_only_shown_days`: setup の工数 a（12 日 10:00–11:00）に加え、表示範囲外（2 日）の工数がある状態で `visible_total_minutes(&days)` が 60。12 日 23:00–翌 0:00 の工数を足すと 120（Review Focus 4）。表示範囲の前日 23:00–24:00 の工数は数えない
  - `selection_label_shows_title_and_span`: entry-a をクリック後 `Some("作業a 10:00–11:00（1:00）")`、Esc 後 None
  - `status_bar_shows_calendar_total`: debug selector `status-total` が `status-bar` の中にある
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-app calendar_tests` / Expected: FAIL
- [ ] **Step 3: 実装**
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test -p track-app` / Expected: PASS
- [ ] **Step 5: Commit** — `feat(app): カレンダーを淡い色の面と状態バーに`

### Task 5: レポート（集計列の削除・割合バー）

**Files:**
- Modify: `gpui/crates/track-app/src/reports/mod.rs`、`reports/custom_output.rs`、`reports/weekly_report.rs`
- Delete: `gpui/crates/track-app/src/reports/summary.rs` のドーナツ・凡例・折りたたみ帯（`colors` など明細表が使う関数は残すか mod.rs へ移す）
- Modify: `gpui/crates/track-app/src/persist.rs`（`summary_collapsed` を削除。`#[serde(default)]` なので古い値は読み捨てられる）
- Modify: `gpui/crates/track-app/src/app.rs`（レポート表示時に状態バーの左へ渡す）
- Test: `gpui/crates/track-app/src/reports_tests.rs`、`persist.rs` のテスト

**Interfaces:**
- Consumes: `palette`, `MONO`, `status_bar::render`
- Produces: 明細の一番上の行の割合列に `share-bar-<key>`（トラック）と `share-fill-<key>`（塗り）の debug selector。`<key>` は行の id（プロジェクト id、`none` など今の行 id と同じもの）

明細表は全幅。一番上の行の割合列を「細いバー（トラック 120px 前後、塗りは行の色）＋ `MONO` の %」にする。子行は今どおり割合なし。行頭の色の点は残す。状態バーの左: 「合計 <時間>」「<N> <プロジェクト|クライアント|タグ>」（`status-total`・`status-rows`）。

- [ ] **Step 1: 失敗するテストを書く**
  - `summary_column_is_gone`: レポートを開き、`donut`・`legend`・`summary-toggle`（今の ID に合わせる）が無い。明細表の左端がウインドウ左端から 16px 以内
  - `share_bar_width_matches_share`: 2 行（90 分・30 分）で、`share-fill-<p1>` の幅 / `share-bar-<p1>` の幅が 0.75 ±0.02、もう一方が 0.25 ±0.02（Review Focus 2）
  - `single_row_bar_is_full`: 1 行だけのとき塗りの幅がトラックと等しい（±1px）
  - `old_persist_file_with_summary_collapsed_loads`（persist.rs）: `{"summaryCollapsed": true, "zoomIndex": 2}` を読むと `zoom_index == 2` で他は既定（Review Focus 1）
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-app reports` / Expected: FAIL
- [ ] **Step 3: 実装**
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test -p track-app` / Expected: PASS
- [ ] **Step 5: Commit** — `feat(app): レポートを明細表と割合バーに`

### Task 6: 設定ウインドウとダイアログ

**Files:**
- Modify: `gpui/crates/track-app/src/settings/mod.rs`、`settings/form.rs`、`settings/calendar.rs`、`settings/reports.rs`、`settings/format_editor.rs`、`settings/projects.rs`、`settings/ai.rs`、`settings/backup.rs`
- Test: `gpui/crates/track-app/src/settings_tests.rs`（既存テストが通ること）

**Interfaces:**
- Consumes: `palette`, `MONO`

配置は変えず、生の色を `Palette` の名前へ置き換える。左の項目一覧は `surface`、選択中の項目は `selected`。数字（勤務時刻・間隔・保存数）は `MONO`。

- [ ] **Step 1: 既存テストを実行して基準を取る** — Run: `cd gpui && cargo test -p track-app settings_tests` / Expected: PASS（置き換え前）
- [ ] **Step 2: 置き換える**
- [ ] **Step 3: 通過を確認** — Run: `cd gpui && cargo test -p track-app` / Expected: PASS
- [ ] **Step 4: Commit** — `feat(app): 設定ウインドウを意味の色に`

### Task 7: 生の色関数の削除・実地確認・ドキュメント

**Files:**
- Modify: `gpui/crates/track-app/src/ui/theme.rs`（`white`・`n50`〜`n900`・`amber*`・`red*`・`blue500`・`header_bg`・`header_border`・`black_a`・`white_a` を削除。`hex`・`mix`・`with_alpha`・`NO_PROJECT` は残す）
- Modify: 残っている呼び出し元すべて
- Modify: `gpui/README.md`、`gpui/NOTES.md`（見た目の方針、Tauri 版との差の一覧を更新。24:00 の編集不具合は修正済みに）

- [ ] **Step 1: 削除して取り残しを出す** — Run: `cd gpui && cargo build -p track-app 2>&1 | grep -c "cannot find function"` / Expected: 0 になるまで呼び出し元を `Palette` へ置き換える
- [ ] **Step 2: 全テストと clippy** — Run: `cd gpui && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` / Expected: PASS、警告なし
- [ ] **Step 3: 実地確認** — コピー DB でアプリを起動し、カレンダー・レポート・設定（カレンダー／レポート／プロジェクト／AI／バックアップ）・編集ダイアログ・プロジェクト選択をウインドウだけ撮影して目で確かめる（撮影は commit しない）。macOS を暗い配色にしていても明るく出ることを確認できれば記録する
- [ ] **Step 4: ドキュメント更新**
- [ ] **Step 5: Commit** — `refactor(app): 生の色関数を削除` と `docs(gpui): Zed 風の見た目`
