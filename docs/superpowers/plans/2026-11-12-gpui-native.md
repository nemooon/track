# Track GPUI ネイティブ版 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** GPUI アプリと Rust 製 CLI が共有する中核クレートで SQLite を直接読み書きする構成に作り変え、Codex/Claude Code 連携・カレンダー・レポートを動かす。

**Architecture:** `gpui/` を Cargo ワークスペースにし、`track-core`（DB・検証・AI 登録・集計・prepare・AI 生成）、`track-cli`（既存 CLI 互換）、`track-app`（GPUI。試作を移設）の3クレートにする。アプリは DB をバックグラウンドで読み書きし、`PRAGMA data_version` で外部変更を検知する。

**Tech Stack:** Rust 1.98.1、`rusqlite`（bundled）、`serde_json`、`sha2`、`chrono`/`chrono-tz`、`uuid`、既存の `gpui-pre =0.3.12` / `gpui-component =0.9.1`

**Spec:** `docs/superpowers/specs/2026-11-12-gpui-native-design.md`

## Global Constraints

- 変更は `gpui/`、`docs/`、`integrations/` の開発用生成物に限る。`src/`、`src-tauri/`、`database/migrations/` は変更しない（マイグレーションは読むだけ）
- DB は `$TRACK_DATA_DIR/track.db`（既定 `~/.track`）。開発ガード: アプリと CLI の書き込み系は `TRACK_DATA_DIR` が `$HOME/.track-gpui` を指す絶対パスでなければ `dev_guard` で拒否（読み取り系 `status`/`projects`/`list` も同じく拒否して安全側に倒す）
- 互換: 日時 TEXT `YYYY-MM-DDTHH:mm:ss.SSS+00:00`（UTC）、真偽 INTEGER 0/1、ID は cuid 形式（`c` + 24 文字の小文字英数）、`_local_migrations(name, appliedAt)`
- 時刻: CLI の同日判定と prepare は JST 固定。アプリとレポート集計はローカル時刻
- CLI の出力: 成功 `{"ok":true,...}`、失敗 `{"ok":false,"error","message","details"}` をどちらも stdout に `JSON.stringify(x, null, 2)` 相当で出す。失敗時の終了コード 1
- テストで本物の `~/.track`、`~/.codex`、`~/.claude` に触れない（ログの場所は環境変数で差し替える）

## Review Focus

1. **Tauri 版で作った DB を開く／Tauri 版が新版の書いた行を読む**: 日時・ID・真偽の形式が既存と同じで、Prisma 側から読める → Task 3 `writes_rows_in_prisma_format` と Task 13 の手動確認
2. **AI 登録の再送**: Tauri（Bun）版で登録した `requestId` を新 CLI で再送しても二重登録にならない → Task 4 `payload_hash_matches_bun_server`
3. **CLI の同日判定が JST、DB 書き込みがローカル時刻**のずれ（マシンが JST 以外のとき）: 既存と同じ挙動を保つ → Task 4 `create_rejects_cross_jst_midnight_even_if_local_same_day`
4. **アプリ表示中に CLI が書き込む**: 数秒以内に画面に出る → Task 9 `external_write_triggers_reload`
5. **DB の構造が新しすぎる**: 書き込みを拒否する → Task 2 `refuses_unknown_migrations`

---

## ファイル構成

```
gpui/
  Cargo.toml                      ワークスペース
  crates/track-core/src/
    lib.rs  db.rs  migrate.rs  guard.rs  ids.rs  time.rs
    model.rs  repo/{entries.rs,refs.rs,settings.rs}  validate.rs
    ai_registration.rs  reports.rs  report/{group,copy,aggregation,cache,weekly,view}.rs（試作から移設）
    prepare/{mod.rs,logs.rs,windows.rs}  ai/{mod.rs,codex.rs,custom.rs,apple.rs}  runtime.rs  integrations.rs
  crates/track-cli/src/main.rs
  crates/track-app/                試作の src/ を移設（api/ は削除し data/ に置き換え）
  scripts/prepare-db.sh  bench/  NOTES.md  README.md
```

---

### Task 1: ワークスペース化と試作の移設

**Files:** Create `gpui/Cargo.toml`（workspace）、`gpui/crates/track-core/{Cargo.toml,src/lib.rs}`、`gpui/crates/track-cli/{Cargo.toml,src/main.rs}`。Move `gpui/src` → `gpui/crates/track-app/src`、旧 `gpui/Cargo.toml` → `gpui/crates/track-app/Cargo.toml`（パッケージ名 `track-app`、バイナリ名 `track-gpui` は維持）。`rust-toolchain.toml` と `.gitignore` は `gpui/` 直下に残す。

- [ ] **Step 1:** `git mv` で移設し、ワークスペースの `[workspace] members = ["crates/*"]`、`[workspace.dependencies]` に共通依存、`[profile.dev.package."*"] opt-level = 2` を移す
- [ ] **Step 2:** Run `cd gpui && cargo test --workspace` → Expected: 試作の 99 件が通る（パス変更のみ）
- [ ] **Step 3:** `bench/measure.py` の `GPUI` パスを `../target/release/track-gpui` のまま動くことを確認（ワークスペースの target は `gpui/target`）
- [ ] **Step 4:** Commit `refactor(gpui): Cargo ワークスペースに分割`

### Task 2: DB 接続・マイグレーション・開発ガード

**Files:** `track-core/src/{db.rs,migrate.rs,guard.rs}`

**Interfaces (Produces):**
- `pub struct Db { conn: rusqlite::Connection, pub path: PathBuf, pub read_only: bool }`
- `Db::open(data_dir: &Path) -> Result<Db, CoreError>`（WAL、`busy_timeout(5s)`、マイグレーション適用、`Settings` 行の補完）
- `Db::data_version(&self) -> Result<i64, CoreError>`
- `pub fn data_dir_from_env() -> PathBuf`（`TRACK_DATA_DIR` → `~/.track`）
- `pub fn dev_guard(data_dir: &Path) -> Result<(), CoreError>`（`$HOME/.track-gpui` の絶対パス以外は `CoreError::Code("dev_guard", msg)`）
- `pub enum CoreError { Code(&'static str, String), Details(&'static str, String, serde_json::Value), Sql(rusqlite::Error), Io(std::io::Error) }` と `fn code(&self) -> &str`
- マイグレーションは `include_str!("../../../../database/migrations/NNNN_*.sql")` の配列（名前はファイル名そのまま）

- [ ] **Step 1: 失敗するテスト**（tempdir の data_dir）
  - `fresh_db_applies_all_15_migrations`: `_local_migrations` が 15 行、`TimeEntry`/`AiRegistration`/`Settings` 表がある、`journal_mode` が `wal`
  - `reopen_is_idempotent`: 2 回目の open で追加適用なし
  - `creates_default_settings_row`: workStart 600, workEnd 1110, workDays "1,2,3,4,5"
  - `refuses_unknown_migrations`: `_local_migrations` に `9999_future.sql` を足して開くと `read_only == true`、書き込み API は `schema_too_new`
  - `dev_guard_accepts_only_home_copy`: `$HOME/.track-gpui` 可、`~/.track-gpui`（文字列のまま）・`/tmp/x/.track-gpui`・`$HOME/.track` は不可
- [ ] **Step 2:** `cargo test -p track-core db::` 失敗確認 → 実装 → 通過
- [ ] **Step 3: 互換確認**（`#[ignore]`）`opens_copy_db_without_migrating`: `$HOME/.track-gpui/track.db` を開いて新規適用 0 件
- [ ] **Step 4:** Commit `feat(core): DB 接続とマイグレーション`

### Task 3: モデルとリポジトリ（参照データ・工数の CRUD）

**Files:** `track-core/src/{model.rs,ids.rs,time.rs,validate.rs,repo/*.rs}`

**Interfaces (Produces):** 試作の `track-app/src/api/types.rs` の型（`TimeEntry`, `Project`, `Client`, `Tag`, `TagLink`, `Settings`, `ReportCopyFormat`…）を `track-core::model` に移し、アプリはそれを使う。
- `ids::cuid() -> String`
- `time::to_db(DateTime<Utc>) -> String`（`%Y-%m-%dT%H:%M:%S%.3f+00:00`）、`time::from_db(&str) -> DateTime<Utc>`（`Z` と `+00:00` の両方を読む）
- `Db::projects(include_archived: bool)`, `clients()`, `tags()`, `settings()`（`reportCopyFormats` が空・不正なら既定「標準」）、`app_config(data_dir)`（`config.json` の `aiProvider` など）
- `Db::entries(from, to) -> Vec<TimeEntry>`（`start < to AND end > from`、start 昇順、project.client・tags 付き）
- `Db::create_entry(NewEntry) -> TimeEntry`、`update_entry(id, EntryPatch)`、`delete_entry(id)`、`entry_titles(project: Option<&str>) -> Vec<String>`
- 検証（`validate.rs`）: 15 分への丸め、ローカル時刻の同日規則（0:00 終了は可）、`end > start`、title ≤100、note ≤500、breakMinutes 0..600、タグ重複除去・ソート。エラーコードは既存サーバーと同じ（`entry_must_be_same_day`、`invalid_rounded_interval`、`invalid_input`、`not_found`）

- [ ] **Step 1: 失敗するテスト**
  - `writes_rows_in_prisma_format`: 作成した行を生 SQL で読むと start が `2026-11-12T01:00:00.000+00:00`、id が `^c[a-z0-9]{24}$`、`createdAt`/`updatedAt` が非 NULL
  - `entries_overlap_query_and_order`、`create_rounds_to_quarter`（10:07 → 10:00）、`create_rejects_cross_day`、`update_replaces_tags`、`delete_removes_tag_links`
  - `settings_falls_back_to_standard_format`
  - `titles_newest_first_distinct_max_100`
- [ ] **Step 2:** 失敗確認 → 実装 → `cargo test -p track-core repo::` 通過
- [ ] **Step 3:** Commit `feat(core): モデルとリポジトリ`

### Task 4: AI 登録（冪等な create）

**Files:** `track-core/src/ai_registration.rs`

**Interfaces (Produces):**
- `pub struct AiRegistrationInput { request_id: String, source: String, session_id: String, allow_overlap: bool }`
- `Db::create_ai_entry(NewEntry, AiRegistrationInput) -> Result<(TimeEntry, bool /*replayed*/), CoreError>`（トランザクション内で再送判定 → 重なり判定 → 作成と `AiRegistration` 行）
- `Db::ai_progress(source, session_id) -> Option<DateTime<Utc>>`、`Db::ai_coverage(source, session_id) -> Vec<(Option<DateTime<Utc>>, DateTime<Utc>)>`
- `payload_hash(&NewEntry, source, session_id) -> String`: JSON のキー順は `projectId, start, end, title, note, externalEventId, externalEventSource, breakMinutes, [tags], source, sessionId`。日時は `YYYY-MM-DDTHH:mm:ss.SSSZ`。`tags` はタグがあるときだけ `{"create":[{"tagId":..}]}`（ID 昇順）

- [ ] **Step 1: 失敗するテスト**
  - `payload_hash_matches_bun_server`: (a) projectId null、01:00–02:00Z、title「設計」、note なし、breakMinutes 0、タグなし、codex/s1 → `5abdabd7df1e611afb6945ba22431d5d2fd38bc320b2b1559bcc968117669b25`。(b) projectId "p1"、note「メモ」、タグ a,b、claude/s2 → `7d15d87f4dbd712db701751612634eebb528bf4c39383743feac980d630d37d2`（いずれも Node の `JSON.stringify` で算出した値）
  - `replay_same_payload_returns_existing`、`replay_different_payload_conflicts`（`request_id_conflict`）、`replay_after_delete_errors`（`registered_entry_deleted`）
  - `exact_duplicate_even_with_allow_overlap`、`overlap_requires_confirmation`、`allow_overlap_creates`
  - `ai_path_rejects_unrounded_times`（`invalid_time_step`）
  - `coverage_returns_recorded_intervals`
- [ ] **Step 2:** 失敗確認 → 実装 → 通過
- [ ] **Step 3:** Commit `feat(core): AI 登録`

### Task 5: レポート集計

**Files:** `track-core/src/reports.rs`、試作の `core/report/*` を `track-core/src/report/` に移設

**Interfaces (Produces):** `Db::report(&ReportQuery) -> ReportResponse`、`Db::report_entries(&ReportQuery) -> ReportEntriesResponse`（形は試作の型のまま）。規則は `src/server/routes/reports.ts` と同じ（範囲、切り詰め、休憩の按分 `round(break × visible/full)`、`__none__`、タグはエントリとプロジェクトの OR、タグ集計で絞り込みがあれば絞ったタグだけ、未タグは絞り込みなしのときだけ「タグなし」、`totalMinutes` は工数ごとに1回、分数は降順）

- [ ] **Step 1: 失敗するテスト**（fixture DB に工数を作って検証）
  - `week_range_sunday_start`、`clamps_to_range_and_prorates_break`、`none_filters`、`tag_filter_matches_entry_or_project_tag`、`group_by_tag_counts_entry_once_in_total`、`project_labels_client_dot_project`
- [ ] **Step 2:** 失敗確認 → 実装 → 通過。移設した試作のテスト（copy/aggregation/cache/weekly/group/view）も通ること
- [ ] **Step 3: 一致確認**（`#[ignore]`）`matches_bun_server_on_copy_db`: コピー DB に Bun API（8787）を起動しておき、今週・今月 × 3 グループの `/api/reports` と `Db::report` の JSON が一致
- [ ] **Step 4:** Commit `feat(core): レポート集計`

### Task 6: prepare（会話ログから候補を作る）

**Files:** `track-core/src/prepare/{mod.rs,logs.rs,windows.rs}`

**Interfaces (Produces):**
- `pub struct PrepareEnv { now: DateTime<Utc>, codex_roots: Vec<PathBuf>, claude_root: PathBuf, cwd: PathBuf, codex_thread_id: Option<String> }`（`from_env()` は `CODEX_SESSIONS_DIR`、`TRACK_CLAUDE_PROJECTS_DIR`、`CODEX_THREAD_ID` を読む。既存 CLI と同じ既定値）
- `prepare_day(db, env, date: Option<&str>, source: &str, idle_minutes: i64) -> Result<serde_json::Value, CoreError>`
- `prepare_session(db, env, source, session_id: Option<&str>, session_file: Option<&Path>, idle_minutes) -> Result<serde_json::Value, CoreError>`
- 出力 JSON のフィールド名・順序・JST の `+09:00` 表記・エラーコードは `src/cli/index.ts` と同じ

- [ ] **Step 1: 失敗するテスト**: `src/cli/index.test.ts` の 13 ケースを、同じ入力ログ（tempdir に JSONL を書く）・同じ期待値で Rust に移す（テスト名は英語 snake_case）
- [ ] **Step 2:** 失敗確認 → 実装 → 通過
- [ ] **Step 3: 一致確認**（`#[ignore]`）`matches_bun_cli_on_real_logs`: 既存 CLI（`bun src/cli/index.ts prepare --date <今日>`、`TRACK_API_BASE` はコピー DB の Bun API）と Rust の出力を、`requestId`（乱数）を除いて比較し一致（ログは本物の `~/.codex` を**読むだけ**）
- [ ] **Step 4:** Commit `feat(core): prepare`

### Task 7: CLI

**Files:** `track-cli/src/main.rs`

- [ ] **Step 1: 失敗するテスト**（`std::process::Command` で `target` のバイナリを起動。`TRACK_DATA_DIR` は tempdir、開発ガードは `TRACK_DEV_GUARD_ROOT` で tempdir を許可する）
  - `help_prints_usage_exit_0`、`unknown_command_is_invalid_command`
  - `status_reports_db_path_and_project_count`
  - `create_requires_confirmed`、`create_rejects_off_quarter`（`invalid_time_step`）、`create_rejects_cross_jst_midnight_even_if_local_same_day`（`TZ=UTC` で起動）
  - `create_with_request_id_is_idempotent`（同じ引数で2回 → 2回目も成功、工数は1件）
  - `create_without_request_id_reports_overlap`（`overlap_requires_confirmation`、`details` に既存の要約）
  - `errors_go_to_stdout_with_exit_1`
- [ ] **Step 2:** 失敗確認 → 実装 → 通過
- [ ] **Step 3:** Commit `feat(cli): Rust 製 track-cli`

### Task 8: アプリのデータ層を core に置き換え

**Files:** `track-app/src/app.rs`、`track-app/src/data/{mod.rs,store.rs}`（新規）。削除: `track-app/src/api/`（client, sse, ndjson, types）、`/health` 判定、接続エラー画面

**Interfaces (Produces):**
- `pub struct Store`（`Entity`）: `db: Arc<Mutex<Db>>`（バックグラウンド実行器で使う）、`data: AppData`、`read_only: bool`、`version: u64`（外部変更で増える）
- `Store::run<T>(&self, f: impl FnOnce(&mut Db) -> Result<T> + Send) -> Task<Result<T>>`
- カレンダー・レポートは `Store` を通して読み書きする。楽観的更新と巻き戻しは残す

- [ ] **Step 1:** 試作のテスト用スタブサーバー（`testing.rs` の `stub_server`/`hanging_server`）を、tempdir の DB に fixture を入れる `open_app_with_db(cx, fixture)` に置き換える。既存の UI テスト（app/calendar/reports）の期待値はそのまま、データの用意だけ書き換える
- [ ] **Step 2:** テストが失敗することを確認（データ層が HTTP のまま）→ 実装 → `cargo test -p track-app` 通過
- [ ] **Step 3:** Commit `refactor(app): データ層を track-core に置き換え`

### Task 9: 外部変更の検知

**Files:** `track-app/src/data/store.rs`

- [ ] **Step 1: 失敗するテスト** `external_write_triggers_reload`: アプリ表示中に、別の `Db` 接続で工数を作成 → 3 秒以内にカレンダーの `cache` に現れる
- [ ] **Step 2:** 実装: 1 秒ごとに `data_version` を確認し、変化したら参照データと表示中の範囲を読み直す
- [ ] **Step 3:** Commit `feat(app): 外部変更を検知して再読み込み`

### Task 10: レポートの見やすさ（10進数・週の転記コピー）

**Files:** `track-core/src/report/view.rs`、`track-app/src/reports/mod.rs`、`track-app/src/persist.rs`

**Interfaces (Produces):** `decimal_hours(minutes) -> String`（小数第2位、末尾の 0 と `.` を落とす。90 → "1.5"、80 → "1.33"、120 → "2"、0 → "0"）、`weekly_copy_text(rows_by_project) -> String`（`名前\t時間` の行、見出しなし、プロジェクトなしは「プロジェクトなし」）。`PersistState::duration_style: DurationStyle`（`Decimal` 既定 / `HoursMinutes`）

- [ ] **Step 1: 失敗するテスト**: `decimal_hours_formats`、`weekly_copy_uses_projects_regardless_of_group_by`（グループがタグでもプロジェクト単位で出る）、`weekly_copy_follows_filters`、UI テスト `weekly_copy_button_writes_clipboard`
- [ ] **Step 2:** 実装: 時間表示を全体（サマリー・明細・合計行）で切り替え、週表示のときツールバーに「週の合計をコピー」ボタン
- [ ] **Step 3:** Commit `feat(app): レポートの10進数表示と週の転記コピー`

### Task 11: AI 生成プロバイダ

**Files:** `track-core/src/ai/{mod.rs,codex.rs,custom.rs,apple.rs}`、`track-app/src/reports/{custom_output.rs,weekly_report.rs}`

**Interfaces (Produces):** `pub trait AiProvider { fn generate(&self, mode: AiMode, input: &str, on_progress: &mut dyn FnMut(AiProgress)) -> Result<String, CoreError>; }`、`provider_from_config(&AppConfig) -> Box<dyn AiProvider + Send>`。プロンプトは `src/server/ai/prompts.ts` と同じ文面。codex は `codex exec --experimental-json --model <m> --sandbox read-only --cd <tmp> --skip-git-repo-check [--output-schema <file>]` を起動し JSONL の `turn.started`/reasoning/`agent_message`/`turn.failed` を読む（10 分でタイムアウト）。custom は stdin にプロンプト、stdout を結果（3 分、出力 2MB 上限）。apple は `/Applications/Track.app/Contents/Resources/TrackAIHelper.app` を `open -n -W --stdout --stderr` で起動

- [ ] **Step 1: 失敗するテスト**: `custom_command_reads_stdout`（`/bin/cat` を使い入力がそのまま返る）、`custom_command_timeout`、`codex_jsonl_parsing`（記録済みの JSONL サンプルから進捗と最終文を取り出す）、`prompt_matches_server_text`
- [ ] **Step 2:** 実装し、カスタム出力と週報の生成をこのプロバイダに切り替える。UI テストは `/bin/cat` 相当のプロバイダを差し込んで通す
- [ ] **Step 3:** Commit `feat(core): AI 生成プロバイダ`

### Task 12: runtime.json と AI 連携のインストール（開発用の別名）

**Files:** `track-core/src/{runtime.rs,integrations.rs}`、`track-app/src/actions.rs`（メニュー）、`gpui/README.md`

**Interfaces (Produces):** `write_runtime(data_dir, cli_path)`（`{pid, cliPath, dbPath, startedAt}` を 0600 で原子的に書く。終了時に消さない）、`install_integrations(kind: Codex|Claude|Both, dev: bool) -> Result<Vec<PathBuf>>`（上書き規則は `src-tauri/src/lib.rs:33-210` と同じ。`dev` のときは `track-dev` の名前で、文面の `~/.track/runtime.json` を `$HOME/.track-gpui/runtime.json` に、`name: track` を `name: track-dev` に、コマンド名を `/track-dev` に置き換える。HTTP 前提のエラー説明と「Track を起動」の案内を新 CLI 向けに直す）

- [ ] **Step 1: 失敗するテスト**（`HOME` を tempdir に差し替え）: `runtime_written_atomically_0600`、`installs_dev_skill_under_track_dev`、`never_touches_real_track_skill_in_dev`、`refuses_to_overwrite_unrelated_files`
- [ ] **Step 2:** 実装し、メニューに「AI 連携をインストール…（開発用）」を追加（Codex / Claude Code / 両方を選ぶダイアログ）
- [ ] **Step 3: 手動確認**: コピー DB で、Codex から `$track-dev` で今日の作業を登録 → GPUI 版のカレンダーに数秒以内に出る
- [ ] **Step 4:** Commit `feat: runtime.json と AI 連携のインストール`

### Task 13: 互換確認・計測・所感

- [ ] **Step 1:** コピー DB を GPUI 版で更新したあと、Tauri 版の同梱サーバー（`TRACK_DATA_DIR=$HOME/.track-gpui`、別ポート）で `/api/entries` と `/api/reports` を読み、新版で作った工数が読めることを確認
- [ ] **Step 2:** `gpui/bench` で起動・メモリ・CPU・週切替を再計測（API サーバーなし）
- [ ] **Step 3:** `gpui/NOTES.md` に「作りを変えて変わったこと」、計測値、本採用に残る作業を追記
- [ ] **Step 4:** `cargo test --workspace && cargo clippy --workspace` が通ることを確認して Commit `docs(gpui): ネイティブ版の所感`
