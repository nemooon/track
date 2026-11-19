# GPUI 版 設定画面 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tauri 版の設定画面（メモを除く）を GPUI 版の別ウインドウとして移し、自動バックアップと復元をアプリに持たせる。

**Architecture:** 規則（入力チェック・削除条件・config.json の正規化・バックアップ）は `track-core` に Rust で移植し、`track-app` は `DataClient` 経由で呼ぶ。設定は `cx.open_window` の 2 つ目のウインドウで、`AppState` を共有して保存後にメイン画面を明示的に読み直す。

**Tech Stack:** Rust 1.98.1、gpui-pre 0.3.12、gpui-component 0.9.1、rusqlite

**Spec:** `docs/superpowers/specs/2026-11-12-gpui-settings-design.md`

## Global Constraints

- 正とする TS: `src/client/pages/{SettingsPage,ProjectsPage,ReportCopyFormatPage}.tsx`、`src/client/components/SettingsLayout.tsx`、`src/server/routes/{settings,config,clients,projects,tags,data}.ts`、`src/server/db/backup.ts`、`src/server/config.ts`、`src/server/ai/models.ts`、`src/shared/validators.ts`。文言・制約はここから写す
- 開発用ガード: DB は `~/.track-gpui` のみ。**開発版ではバックアップ保存先・復元元も `~/.track-gpui` 配下のみ**（`guard::allowed_root()` 配下）。iCloud 候補は出さない
- 本物の `~/.track`、iCloud Drive の `Track` フォルダ、`~/.codex/skills/track`、`~/.claude/commands/track.md` に触れない。実データでの AI 生成をしない
- `config.json` は知らないキーを残して保存する。`TRACK_EXPORT_DIR` は見ない
- スクリーンショットは `gpui/scripts/winshot.sh` でアプリのウインドウだけ。コミットしない
- コミット末尾に `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`
- テストは `cd gpui && cargo build --workspace && cargo test --workspace`、各タスクの終わりに `cargo clippy --workspace --all-targets` を警告ゼロ

## Review Focus

1. **Tauri 版と同じ config.json を往復**: GPUI 版が保存したあとも Tauri 版の `loadConfig` が同じ値を読み、知らないキー（例 `exportDir` 以外の将来キー）が消えない → Task 3 `save_config_keeps_unknown_keys`
2. **復元の途中失敗**: 退避バックアップが作れないときは現在の DB を置き換えない → Task 4 `restore_aborts_when_safety_backup_fails`
3. **古いスナップショット（`User` テーブル時代）からの復元**: 受け入れてマイグレーションで現行化する → Task 4 `restore_old_snapshot_migrates`
4. **開発版でバックアップ先に本物の場所を指定**: 保存を拒否し、間引きも走らない → Task 3 `dev_export_dir_outside_root_is_rejected`
5. **設定保存がメイン画面に即反映**: 勤務時間・プロジェクト追加・出力フォーマットがメインウインドウに出る → Task 6 `saving_settings_refreshes_main_window`

---

### Task 1: クライアント・プロジェクト・タグの更新と削除（core）

**Files:** Modify `gpui/crates/track-core/src/repo/refs.rs`、`model.rs`。Test `gpui/crates/track-core/src/repo/refs_tests.rs`（新規、`repo/mod.rs` に `#[cfg(test)] mod refs_tests;`）

**Interfaces (Produces):**
- `Db::clients_all() -> Result<Vec<Client>>`（アーカイブ含む、createdAt 昇順）、既存 `projects(include_archived)`、`tags()`（name 昇順）
- `ClientPatch { name: Option<String>, archived: Option<bool> }`、`ProjectPatch { name, color, client_id, archived: Option<_>, tag_ids: Option<Vec<String>> }`、`TagPatch { name, color }`（`model.rs`）
- `Db::update_client(&mut self, id, ClientPatch) -> Result<Client>`、`delete_client(id)`、`update_project(id, ProjectPatch) -> Result<Project>`、`delete_project(id)`、`update_tag(id, TagPatch) -> Result<Tag>`、`delete_tag(id)`
- 既存 `create_client/create_project/create_tag` に入力チェックを足す（シグネチャは維持。`create_project` に `tag_ids: &[String]` を足した `create_project_with_tags` を追加）

規則は TS どおり: 名前 trim なし・1〜100 字（タグ 1〜50）、色 `^#[0-9a-fA-F]{6}$` → `invalid_input`。存在しない id → `not_found`。タグ名重複 → `tag_already_exists`。クライアントを `archived: true` にすると配下プロジェクトも同じトランザクションでアーカイブ。工数のあるプロジェクトの削除 → `project_has_entries`。クライアント・タグの削除は DB の外部キーに任せる。書き込みは全て `ensure_writable()`。

- [ ] **Step 1: 失敗するテスト**: `archiving_client_archives_its_projects`、`project_with_entries_cannot_be_deleted`（code `project_has_entries`、工数を消すと削除できる）、`duplicate_tag_name_is_rejected`（`tag_already_exists`）、`invalid_color_and_empty_name_are_rejected`（`"#12345"`、`""`、101 字 → `invalid_input`）、`update_unknown_id_is_not_found`、`project_tag_ids_replace_existing`、`deleting_client_cascades_like_prisma`（スキーマの外部キーどおり、削除後に `projects(true)` から消える）、`writes_refused_when_schema_too_new`
- [ ] **Step 2:** `cargo test -p track-core refs_tests` → コンパイルエラーで FAIL
- [ ] **Step 3:** 実装
- [ ] **Step 4:** 同コマンドで PASS
- [ ] **Step 5:** Commit `feat(core): クライアント・プロジェクト・タグの更新と削除`

### Task 2: 勤務設定と出力フォーマットの保存（core）

**Files:** Modify `gpui/crates/track-core/src/repo/settings.rs`、`model.rs`。Test `gpui/crates/track-core/src/repo/settings_tests.rs`（新規）

**Interfaces (Produces):** `SettingsPatch { work_start: Option<i64>, work_end: Option<i64>, work_days: Option<Vec<u32>>, weekly_report_template: Option<String>, report_copy_formats: Option<Vec<ReportCopyFormat>> }`、`Db::update_settings(&mut self, SettingsPatch) -> Result<Settings>`、`validate_report_copy_format(&ReportCopyFormat) -> Result<()>`（`report/copy.rs` か `validate.rs`）

規則は `settingsUpdateSchema` と `PATCH /api/settings` どおり: 分は 0〜1440、曜日 0〜6、`workEnd <= workStart` か曜日が空 → `invalid_schedule`。テンプレートは trim 後 1〜10,000 字。フォーマットは 1〜20 件、各 `name` trim 1〜100、列 1〜30、`target` に応じた許可フィールド（entries / ai-aggregation の集合は validators.ts から写す）、AI 列は ai-aggregation のみ → `invalid_input`。`reportCopyFormats` は JSON 文字列で保存（キー順は TS の `JSON.stringify` と同じ camelCase）。

- [ ] **Step 1: 失敗するテスト**: `update_settings_partial_keeps_other_fields`、`end_before_start_is_invalid_schedule`、`empty_work_days_is_invalid_schedule`、`formats_roundtrip_in_tauri_json_shape`（保存した文字列を `serde_json::Value` にして `includeHeader`・`aiPrompt`・`durationFormat` のキー名を確認、`settings()` で同じ値が返る）、`ai_column_only_for_aggregation`、`field_not_allowed_for_target_is_rejected`、`blank_template_is_rejected`
- [ ] **Step 2:** FAIL を確認 → **Step 3:** 実装 → **Step 4:** PASS
- [ ] **Step 5:** Commit `feat(core): 勤務設定と出力フォーマットの保存`

### Task 3: config.json の読み書き（core）

**Files:** Create `gpui/crates/track-core/src/config.rs`（`repo::app_config` はこれに置き換えて削除し、呼び出し元を直す）。Test 同ファイル `#[cfg(test)] mod tests`

**Interfaces (Produces):**
- `pub struct Config { export_dir: PathBuf, backup_interval_hours: i64, backup_keep: i64, ai_provider: String, ai_codex_executable, ai_codex_model, ai_command_executable: String, ai_command_args: Vec<String> }` と `Config::defaults(data_dir)`（`exports`、24、30、`apple-intelligence`、空）
- `load_config(data_dir) -> Config`（`loadConfig` と同じ正規化。`TRACK_EXPORT_DIR` は見ない）
- `ConfigPatch`（全フィールド `Option`）と `save_config(data_dir, ConfigPatch) -> Result<Config>`: `updateSchema` の範囲チェック（間隔 0〜720、保存数 1〜1000、引数 ≤32 個・各 1〜2000 字 など）→ `invalid_input`。`export_dir` 指定時は絶対パス（`not_absolute`）、作成して書けるか（`not_writable`）、**開発版ガード**: `guard::allowed_root()` 配下でなければ `dev_guard`。ファイルは既存 JSON を読み、知らないキーを残してマージし、整形 JSON＋改行で原子的に書く
- `export_dir_suggestions(home) -> Vec<(String, PathBuf)>`: iCloud Drive が存在すれば `("iCloud Drive", …/Track)`。**開発版（ガード有効中）は空を返す**
- `AppConfig` は `Config` から作る（`impl From<&Config> for AppConfig`）か `Config` に統合する。`ai::provider_from_config` の呼び出しはそのまま動くこと

- [ ] **Step 1: 失敗するテスト**: `load_config_normalizes_like_tauri`（`backupKeep: 0`→30、`aiProvider: "x"`→既定、引数の空文字除去と trim）、`save_config_keeps_unknown_keys`（`{"futureKey":1}` が残る）、`dev_export_dir_outside_root_is_rejected`（`TRACK_DEV_GUARD_ROOT` を tempdir にし、外のパス → `dev_guard`、ファイル不変）、`relative_export_dir_is_rejected`、`interval_and_keep_ranges`、`suggestions_hidden_in_dev`
- [ ] **Step 2:** FAIL → **Step 3:** 実装 → **Step 4:** PASS（既存の ai・CLI テストも通ること）
- [ ] **Step 5:** Commit `feat(core): config.json の読み書き`

### Task 4: バックアップと復元（core）

**Files:** Create `gpui/crates/track-core/src/backup.rs`、Modify `db.rs`（必要なら `Db::reopen`）。Test `gpui/crates/track-core/src/backup_tests.rs`

**Interfaces (Produces):** `SnapshotInfo { name, path: PathBuf, bytes: u64, created_at: DateTime<Utc>, auto: bool }`、`list_snapshots(dir) -> Vec<SnapshotInfo>`（新しい順、`track-auto-`/`track-backup-` の `.db` のみ）、`Db::snapshot(&self, dir, auto) -> Result<SnapshotInfo>`（`VACUUM INTO`、名前は `track-{auto|backup}-YYYYMMDD-HHMMSS[-n].db` ローカル時刻）、`prune_snapshots(dir, keep) -> usize`（自動分のみ）、`Db::maybe_auto_backup(&self, &Config) -> Result<Option<SnapshotInfo>>`、`validate_snapshot(path) -> Result<SnapshotCounts, String>`（読み取り専用で開く、`integrity_check`、必須テーブル Client/Project/TimeEntry/Tag ＋ Settings か User、件数）、`Db::restore(&mut self, path, safety_dir) -> Result<RestoreResult { safety: SnapshotInfo, migrated: Vec<String> }>`

手順は `backup.ts` の `restore` どおり: 検証 → 現在の DB を `safety_dir` へ手動バックアップ（**失敗したら中止**）→ 一時コピー → 接続を閉じる → `-wal`/`-shm` 削除 → rename → `migrate` → 開き直し（`self` を新しい接続に入れ替える）。ガード: `dir`・`path`・`safety_dir` が開発版の許可範囲外なら `dev_guard`。

- [ ] **Step 1: 失敗するテスト**: `snapshot_is_consistent_single_file`（`-wal` なしで開けて件数一致）、`prune_keeps_manual_backups`、`auto_backup_respects_interval`（直後の 2 回目は `None`、間隔 0 は常に `None`）、`validate_rejects_non_track_db`、`restore_replaces_and_reopens`（復元後の `entries()` が復元元の内容）、`restore_aborts_when_safety_backup_fails`（`safety_dir` を書き込み不可にして Err、元の DB の内容が残る）、`restore_old_snapshot_migrates`（`User` テーブルの古い構造を手で作り、復元後に `Settings` が読める）、`backup_dir_outside_dev_root_is_rejected`
- [ ] **Step 2:** FAIL → **Step 3:** 実装 → **Step 4:** PASS
- [ ] **Step 5:** Commit `feat(core): バックアップと復元`

### Task 5: AI の状態表示とモデル候補（core）

**Files:** Modify `gpui/crates/track-core/src/ai/{mod.rs,codex.rs,custom.rs,apple.rs}`、Create `ai/models.rs`。Test `ai/tests.rs` に追記

**Interfaces (Produces):** `AiProvider::status(&self) -> AiStatus { provider: &'static str, label: String, available: bool, detail: String }`（文言は `providers.ts` の `status()` どおり）、`codex_models(executable: &str, home: &Path) -> CodexModelCatalog { models: Vec<(String /*id*/, String /*label*/)>, source: "codex"|"cache"|"fallback", detail: String }`（`app-server` の JSON-RPC `initialize`→`initialized`→`model/list` ページング、15 秒、2MB 上限 → 失敗時 `$CODEX_HOME/models_cache.json` の `visibility == "list"` → 固定候補 4 件。文言は models.ts どおり）

- [ ] **Step 1: 失敗するテスト**: `status_texts_match_server`（custom 未設定／設定済み、codex 実行ファイルなし）、`codex_models_from_app_server`（偽 codex スクリプトが JSON-RPC に応答し 2 ページ返す、hidden は除外）、`codex_models_fall_back_to_cache`、`codex_models_fall_back_to_builtin`
- [ ] **Step 2:** FAIL → **Step 3:** 実装 → **Step 4:** PASS
- [ ] **Step 5:** Commit `feat(core): AI の状態表示とモデル候補`

### Task 6: 設定ウインドウの骨組み

**Files:** Create `gpui/crates/track-app/src/settings/{mod.rs,window.rs}`、Modify `actions.rs`（`OpenSettings`、`⌘,`、Track メニュー「設定…」）、`app.rs`（ヘッダー右に設定ボタン、`AppState::refresh_all`）、`data/mod.rs`（Task 1〜5 の操作を `call` で公開）、`main.rs`。Test `gpui/crates/track-app/src/settings_tests.rs`

**Interfaces (Produces):** `SettingsSection { Calendar, Reports, Projects, Ai, Backup }`（表示名「カレンダー」「レポート」「プロジェクト」「AI」「バックアップと復元」、グループ見出しは SettingsLayout の「ビュー／共通／データ」）、`open_settings(state: Entity<AppState>, cx: &mut App) -> WindowHandle<…>`（既に開いていれば前面に出して同じハンドル）、`SettingsWindow { pub section: SettingsSection }`、`AppState::refresh_all(cx)`（参照データ＋設定＋config を読み直し、メイン画面の各ページに再読み込みさせる）。各セクションは後続タスクで中身を入れる（ここでは見出しだけ）

- [ ] **Step 1: 失敗するテスト**: `cmd_comma_opens_single_settings_window`（2 回押してもウインドウは 1 つ）、`header_button_opens_settings`、`sidebar_switches_sections`、`saving_settings_refreshes_main_window`（設定側から `update_settings` で勤務開始を変え `refresh_all` → メイン側 `AppState.data.settings.work_start` が変わる）
- [ ] **Step 2:** FAIL → **Step 3:** 実装 → **Step 4:** PASS
- [ ] **Step 5:** Commit `feat(app): 設定ウインドウ`

### Task 7: カレンダーとレポート（週報テンプレート）の項目

**Files:** Create `settings/calendar.rs`、`settings/reports.rs`（テンプレート部分）。Test `settings_tests.rs`

勤務時間（開始・終了の時刻入力、15 分刻みの選択肢）、勤務日（日〜土のトグル）、週報テンプレート（複数行入力）。保存ボタン・「変更を破棄」・「保存済み」表示・エラー表示は SettingsPage の挙動どおり。保存成功で `refresh_all`。

- [ ] **Step 1: 失敗するテスト**: `work_schedule_save_and_discard`、`invalid_schedule_shows_error_and_keeps_db`、`weekly_template_saves`
- [ ] **Step 2〜4:** FAIL → 実装 → PASS
- [ ] **Step 5:** Commit `feat(app): 設定のカレンダーと週報テンプレート`

### Task 8: プロジェクトの項目

**Files:** Create `settings/projects.rs`。Test `settings_tests.rs`

`ProjectsPage` どおり: クライアントごとにプロジェクトを並べる、クライアント追加、プロジェクト追加（名前＋色。色の選択肢は ProjectsPage のパレット）、名前・色の変更、アーカイブ（「アーカイブ済みを表示」）、削除（`project_has_entries` の文言を表示）、タグの追加・変更・削除。変更のたびに `refresh_all`。

- [ ] **Step 1: 失敗するテスト**: `add_client_then_project_with_color`（利用者の普段の流れ。追加後メイン画面のプロジェクト候補に出る）、`archive_hides_project_until_toggle`、`delete_project_with_entries_shows_message`、`tag_crud`
- [ ] **Step 2〜4:** FAIL → 実装 → PASS
- [ ] **Step 5:** Commit `feat(app): 設定のプロジェクト管理`

### Task 9: 出力フォーマットの一覧と編集

**Files:** Create `settings/report_formats.rs`（一覧）、`settings/format_editor.rs`。Test `settings_tests.rs`

`SettingsPage` の出力フォーマット一覧（追加・複製・並べ替え・削除、最低 1 件）と `ReportCopyFormatPage` の編集（名前、対象、区切り、見出し行、AI 指示、列の追加・削除・並べ替え、列ごとの項目・時間書式・AI 列の指示、プレビュー）。保存は `update_settings(report_copy_formats)`。編集画面を設定ウインドウ内の遷移にするかダイアログにするかは実装者の判断。

- [ ] **Step 1: 失敗するテスト**: `add_format_and_edit_columns`、`reorder_and_delete_formats_keeps_at_least_one`、`ai_column_unavailable_for_entries_target`、`saved_format_appears_in_custom_output`（メイン画面のカスタム出力の選択肢に出る）
- [ ] **Step 2〜4:** FAIL → 実装 → PASS
- [ ] **Step 5:** Commit `feat(app): 出力フォーマットの編集`

### Task 10: AI の項目

**Files:** Create `settings/ai.rs`。Test `settings_tests.rs`

プロバイダー選択、Codex（モデル：候補＋自由入力、実行ファイル）、カスタムコマンド（実行ファイル、引数の行追加・削除）、状態表示（`status()`）。モデル候補は表示中 5 分ごとに更新（models.ts の文言どおり）。保存は `save_config`。

- [ ] **Step 1: 失敗するテスト**: `switch_provider_and_save_config`（config.json の `aiProvider` が変わり、他のキーが残る）、`custom_command_args_rows`、`status_reflects_saved_config`
- [ ] **Step 2〜4:** FAIL → 実装 → PASS
- [ ] **Step 5:** Commit `feat(app): 設定の AI`

### Task 11: バックアップと復元の項目、自動バックアップ

**Files:** Create `settings/backup.rs`、Modify `app.rs`（起動時と 1 時間ごとに `maybe_auto_backup` を DB スレッドで実行）、`data/mod.rs`（`restore` は DB スレッド内で接続を入れ替え、完了後 `refresh_all`）。Test `settings_tests.rs`、`app_tests.rs`

保存先フォルダ（`prompt_for_paths` でフォルダ選択、候補は `export_dir_suggestions`）、実行間隔、保存数、「今すぐバックアップ」、復元ポイント一覧（自動／手動、「最新」バッジ、サイズ・日時）、復元の確認ダイアログ（文言は SettingsPage どおり）。

- [ ] **Step 1: 失敗するテスト**: `auto_backup_runs_on_start`（間隔 24・既存なし → 起動後に `track-auto-*.db` が 1 つ）、`backup_now_lists_snapshot`、`restore_from_settings_reloads_main_window`（別内容のスナップショットを復元するとメイン画面のカレンダーがその内容になる）、`export_dir_outside_dev_root_shows_error`
- [ ] **Step 2〜4:** FAIL → 実装 → PASS
- [ ] **Step 5:** Commit `feat(app): バックアップと復元`

### Task 12: 手動確認と所感

- [ ] **Step 1:** コピー DB で release ビルドを起動し、`⌘,` → 各項目を一通り操作（プロジェクト追加→メイン画面の候補に出る、勤務時間変更→網掛けが変わる、今すぐバックアップ→復元）。ウインドウだけスクリーンショット（コミットしない）
- [ ] **Step 2:** GPUI 版が保存した `~/.track-gpui/config.json` と `Settings` を、Bun サーバー（`env -u TRACK_EXPORT_DIR TRACK_DATA_DIR="$HOME/.track-gpui" TRACK_PORT=8797 bun src/server/index.ts`）の `/api/config`・`/api/settings` で読めることを確認
- [ ] **Step 3:** `gpui/NOTES.md` と `gpui/README.md` に設定画面・自動バックアップを追記
- [ ] **Step 4:** Commit `docs(gpui): 設定画面の所感`
