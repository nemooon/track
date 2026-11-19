# GPUI 版 プロジェクト画面の作り直し Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 設定の「プロジェクト」を、クライアントでまとめた一覧（左）と、選んだ項目の詳細（右、すぐ保存）に作り直し、タグを別の項目へ移す。

**Architecture:** `settings/projects.rs` を一覧＋詳細のビューに置き換え、選択中の項目（プロジェクト／クライアント）を状態に持つ。保存は既存の `DataClient` の update/create をそのまま呼ぶ。タグは `settings/tags.rs`（新しい設定項目）へ移す。今月の時間と最終記録日は track-core の新しい集計で取る。

**Tech Stack:** Rust 1.98.1, GPUI (gpui-pre 0.3.12), gpui-component 0.9.1

**Spec:** `docs/superpowers/specs/2026-11-14-gpui-projects-page-design.md`

## Global Constraints

- 変更はすぐ保存。色・クライアント・アーカイブは押した時点、名前は Enter か入力欄から離れたとき。空の名前は保存せず元に戻す
- 保存に失敗したら詳細にエラーを出し、表示は保存済みの値に戻す
- 規則（名前の長さ・色の形式・アーカイブの連鎖・タグ名の重複）は track-core の既存関数のまま
- 色は 20 色（`PROJECT_COLORS`）とスポイト（`settings::color_field::eyedropper`）
- 色は `Palette` だけを使う（`no_raw_hex_colours_outside_the_palette` を通す）
- 本物の `~/.track` に触れない

## Review Focus

1. 名前を空にして離れる → 保存されず元の名前に戻る（Task 3 で固定）
2. 保存に失敗する → エラーが出て、表示は保存済みの値（Task 3 で固定）
3. CLI や AI が選択中のプロジェクトを変更・アーカイブする → 一覧と詳細が追従し、選択は保たれる（アーカイブなら畳んだ欄の中で選ばれたまま）（Task 3 で固定）
4. クライアントをアーカイブ → 配下のプロジェクトもアーカイブの欄へ（Task 3 で固定）
5. クライアントにプロジェクトが 1 つもない → 見出しだけ出て、＋で追加できる（Task 3 で固定）

---

### Task 1: プロジェクトごとの時間と最終記録日（core）

**Files:**
- Modify: `gpui/crates/track-core/src/repo/refs.rs`
- Modify: `gpui/crates/track-core/src/model.rs`
- Modify: `gpui/crates/track-app/src/data/mod.rs`（`project_activity` の呼び出し）
- Test: `gpui/crates/track-core/src/repo/refs_tests.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProjectActivity { pub project_id: String, pub minutes: i64, pub last_start: Option<DateTime<Utc>> }
impl Db {
    /// プロジェクトごとの [from, to) の正味の分（休憩を引く）と、期間によらない最新の工数の開始
    pub fn project_activity(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<ProjectActivity>>;
}
// DataClient::project_activity(from, to) -> impl Future<Output = DataResult<Vec<ProjectActivity>>>
```

- [ ] **Step 1: 失敗するテストを書く** — `project_activity_sums_the_range_and_finds_the_last_entry`: p1 に 11/12 10:00–11:00（休憩 15 分）と 10/30 9:00–10:00、p2 は工数なし → 11 月の範囲で p1 = 45 分・last_start = 11/12 10:00、p2 = 0 分・None（工数のないプロジェクトも返す）
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-core project_activity` / Expected: FAIL（未定義）
- [ ] **Step 3: 実装**（1 本の SQL。範囲内の分は条件付き SUM）
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test -p track-core` / Expected: PASS
- [ ] **Step 5: Commit** — `feat(core): プロジェクトごとの時間と最終記録日`

### Task 2: タグを別の設定項目へ

**Files:**
- Create: `gpui/crates/track-app/src/settings/tags.rs`
- Modify: `gpui/crates/track-app/src/settings/mod.rs`（`SettingsSection::Tags`、「共通」の「プロジェクト」の下、key `tags`、アイコンは同梱の中から）
- Modify: `gpui/crates/track-app/src/settings/projects.rs`（タグの一覧と編集を外す）
- Test: `gpui/crates/track-app/src/settings_tests.rs`（`tag_crud` を「タグ」項目経由に）

**Interfaces:**
- Produces: `settings-nav-tags`。タグの中身の debug selector は今のもの（`tag-add`・`tag-row-*`・`tag-edit`・`tag-delete-*`・`tag-delete-confirm`・`editor-*`）をそのまま使う

- [ ] **Step 1: テストを直す** — `tag_crud` が `open_section(cx, "settings-nav-tags")` から始まるようにし、`tags_are_not_on_the_projects_page`: 「プロジェクト」を開いても `tag-add` が無い
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-app settings_tests::tag` / Expected: FAIL（`settings-nav-tags` が無い）
- [ ] **Step 3: 実装**（今のタグ部分をそのまま移す）
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test -p track-app` / Expected: PASS
- [ ] **Step 5: Commit** — `feat(app): タグを設定の別項目に移す`

### Task 3: 一覧＋詳細のプロジェクト画面

**Files:**
- Modify: `gpui/crates/track-app/src/settings/projects.rs`（作り直し）
- Test: `gpui/crates/track-app/src/settings_tests.rs`（プロジェクトの既存テスト `add_client_then_project_with_color`・`archive_hides_project_until_restored`・`projects_section_follows_outside_changes`・色のテスト群を新しい画面に書き換え）

**Interfaces:**
- Consumes: Task 1 の `DataClient::project_activity`、`color_field::eyedropper`、`PROJECT_COLORS`
- Produces（debug selector）:
  - 一覧: `tree-client-{id}`（見出し、押すとクライアントを選ぶ）、`tree-client-add-{id}`（＋）、`tree-project-{id}`、`tree-add-client`、`tree-archived`（畳んだ欄の開閉）
  - 詳細（プロジェクト）: `detail-name`（入力）、`detail-client-{id}`（クライアントの選択肢）、`detail-color-{i}`、`editor-color-custom`（スポイト、既存）、`detail-activity`、`detail-archive`、`detail-error`
  - 詳細（クライアント）: `detail-name`、`detail-project-count`、`detail-archive`
  - 何も選んでいない: `detail-empty`

```rust
pub enum Selection { Project(String), Client(String) }
// ProjectSettings に pub selected: Option<Selection>
```

挙動:
- 開いたとき、アーカイブされていない最初のプロジェクトを選ぶ（無ければ `detail-empty`）
- ＋（クライアント）: 名前「新しいプロジェクト」・色は `next_project_color(プロジェクト数)` で作成 → その行を選び、名前欄にフォーカスして全選択
- ＋ クライアント: 名前「新しいクライアント」で作成 → 選び、名前欄にフォーカス
- 名前: Enter か blur で保存。空なら保存せず元に戻す
- 色・クライアント・アーカイブ: 押した時点で保存
- 保存失敗: `detail-error` にメッセージ、表示は保存済みの値（再読み込み）
- 外部の変更（`StateEvent::Reloaded`）: 一覧と詳細を読み直し、選択は id で保つ。消えていたら選択を外す
- アーカイブ済みのクライアント・プロジェクトは `tree-archived` の中（初期は畳む。選択中のものがアーカイブされたら開く）
- 今月の時間 `detail-activity`: 「今月 12h 30m・最終記録 11/12」の形（記録が無ければ「記録なし」）

- [ ] **Step 1: 失敗するテストを書く**（古いプロジェクト画面のテストは消して以下に置き換える）
  - `opens_with_the_first_project_selected`
  - `add_project_under_a_client_selects_it_and_saves_a_name_on_enter`: `tree-client-add-c1` → 新しい行が選ばれる → `detail-name` に「新案件」を入れて Enter → DB の名前が「新案件」、クライアントは c1、色は `PROJECT_COLORS` の 1 つ
  - `colour_and_client_save_immediately`: `detail-color-3` → DB の色が `PROJECT_COLORS[3]`。`detail-client-{別}` → DB のクライアントが変わる
  - `empty_name_is_not_saved`（Review Focus 1）
  - `failed_save_shows_error_and_reverts`（Review Focus 2）: `fail_ops(&["update_project"])` → 色を押す → `detail-error` が出て、詳細の色の選択は元の色
  - `outside_change_keeps_the_selection`（Review Focus 3）: 別接続で p1 の名前を変えて refresh → 選択は p1 のまま、`detail-name` の値が新しい名前
  - `archiving_a_client_moves_its_projects_to_the_archive`（Review Focus 4）
  - `empty_client_shows_its_add_button`（Review Focus 5）: プロジェクトの無いクライアントの見出しと `tree-client-add-{id}` がある
  - `picker_colour_saves`（既存の色テストの置き換え）: スポイトの Change で `#123456` が保存される
  - `activity_shows_month_and_last_entry`
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-app settings_tests` / Expected: FAIL（新しい id が無い）
- [ ] **Step 3: 実装**
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings` / Expected: PASS、警告なし
- [ ] **Step 5: Commit** — `feat(app): プロジェクト画面をクライアントでまとめた一覧と詳細に作り直す`

### Task 4: 実地確認とドキュメント

- [ ] **Step 1: コピー DB で起動し、プロジェクト画面とタグ画面をウインドウだけ撮影して確認**（撮影は commit しない。操作はクリックの座標に気をつける）
- [ ] **Step 2: `gpui/README.md` の設定の節と `gpui/NOTES.md` を更新**
- [ ] **Step 3: 全テスト** — Run: `cd gpui && cargo test --workspace` / Expected: PASS
- [ ] **Step 4: Commit** — `docs(gpui): プロジェクト画面の作り直し`
