# GPUI 版 AI からの編集と取り消し Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `track-cli` から工数の一括変更・削除とクライアント／プロジェクト／タグの操作をできるようにし、AI の操作を会話（`undo`）と GPUI 画面（［元に戻す］）の両方から取り消せるようにする。

**Architecture:** track-core に「操作（AiOperation）」の記録層を足す。1 コマンド = 1 トランザクション = 1 操作で、影響する行の範囲（スコープ: テーブル・列・値）ごとに変更前後の行を JSON で保存する。取り消しは「現在の行 = 記録した変更後」を全スコープで確かめてから変更前へ戻す。CLI は確認ルール（`--confirmed`）を強制し、GPUI アプリは外部変更の検知時に新しい操作を表示する。

**Tech Stack:** Rust 1.98.1, rusqlite, serde_json, GPUI (gpui-pre 0.3.12) + gpui-component 0.9.1

**Spec:** `docs/superpowers/specs/2026-11-12-gpui-ai-editing-design.md`

## Global Constraints

- 開発中は DB を `~/.track-gpui` 以外で開かない（既存の dev guard のまま）。テストは一時ディレクトリだけを使う
- 共有の `integrations/` の文面は変えない（Tauri 版 CLI に無いコマンドを書くことになるため）。GPUI 版の追加分は track-core 内の文面を `adapt` で付け足す
- 確認が要る操作: 工数 **10 件以上**の変更、工数の**削除**（件数によらず）、**クライアント・プロジェクトの作成**、**タグの削除**。`--confirmed` が無ければ `confirmation_required` で止める
- 1 コマンド = 1 操作。途中で失敗したら何も変えず、操作も記録しない
- 取り消しは AI（CLI）の操作だけ。1 行でも「変更後」と違えば `undo_conflict` で全体を止める。同じ操作は 2 回取り消せない（`already_undone`）
- 記録の保持: 新しい順に 100 件を超えた分と、30 日より古い分を消す
- クライアント・プロジェクトは削除しない（アーカイブのみ）。設定・バックアップ・復元は対象外
- 出力は既存 CLI と同じ `{ "ok": true, ... }` / `{ "ok": false, "error", "message", "details"? }`。メッセージは日本語

## Review Focus

1. 取り消したい操作のあとに、**別の AI 操作が同じ行を変えていた**とき → `undo_conflict` で止まり、何も変わらない（Task 3 で固定）
2. 一括変更の ID の中に**存在しないもの**が混じるとき → 1 件も変わらず `not_found`、操作も残らない（Task 4 で固定）
3. **クライアントのアーカイブ**（配下プロジェクトも連鎖でアーカイブ）を取り消すと、プロジェクトのアーカイブ状態も元に戻る（Task 4 で固定）
4. 画面と CLI から**同じ操作を続けて取り消す** → 2 回目は `already_undone`（Task 3 で固定）
5. `search --query` に `%` や `_` を含む語 → 文字どおりに一致する（LIKE のワイルドカードとして扱わない）（Task 4 で固定）

---

### Task 1: 操作テーブルと入れ子にできる書き込み

**Files:**
- Create: `database/migrations/0016_ai_operation.sql`
- Modify: `gpui/crates/track-core/src/migrate.rs`（`MIGRATIONS` に追加）
- Modify: `gpui/crates/track-core/src/repo/entries.rs`, `repo/refs.rs`, `ai_registration.rs`（`transaction()` を入れ子可能に）
- Test: `gpui/crates/track-core/src/ops_tests.rs`（新規。`lib.rs` に `#[cfg(test)] mod ops_tests;`）

**Interfaces:**
- Produces: テーブル `AiOperation`。Db の書き込みメソッドは、外側のトランザクションの中から呼んでも動く

```sql
CREATE TABLE "AiOperation" (
  "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
  "createdAt" DATETIME NOT NULL,
  "source" TEXT NOT NULL,
  "summary" TEXT NOT NULL,
  "count" INTEGER NOT NULL,
  "changes" TEXT NOT NULL,
  "undoneAt" DATETIME
);
CREATE INDEX "AiOperation_createdAt_idx" ON "AiOperation" ("createdAt");
```

- [ ] **Step 1: 失敗するテストを書く** — `writes_nest_inside_an_outer_transaction`: `db.conn().execute_batch("BEGIN IMMEDIATE")` のあと `create_client`・`create_project_with_tags`・`update_entry`・`update_client`・`delete_tag`・`create_ai_entry` を呼んでエラーにならないこと、`ROLLBACK` 後にどれも残っていないこと。`migration_adds_ai_operation_table`: `Db::open` 後に `SELECT COUNT(*) FROM "AiOperation"` が 0
- [ ] **Step 2: 実行して失敗を確認** — Run: `cd gpui && cargo test -p track-core ops_tests` / Expected: FAIL（`cannot start a transaction within a transaction` と no such table）
- [ ] **Step 3: 実装** — migration を足す。各書き込みメソッドの `transaction()` / `transaction_with_behavior(Immediate)` を `savepoint()` に替える（rusqlite の `Savepoint` は外側が無ければトランザクションになる）。`create_ai_entry` は外側が無いとき（`conn.is_autocommit()`）だけ先に `BEGIN IMMEDIATE` 相当を取り、同時実行の守りを保つ。`insert_entry` は `&rusqlite::Connection` を受ける形にしてよい
- [ ] **Step 4: 全テスト** — Run: `cd gpui && cargo test -p track-core` / Expected: PASS（既存 126 + 新規）
- [ ] **Step 5: Commit** — `feat(core): 操作記録テーブルと入れ子にできる書き込み`

### Task 2: 操作の記録（スコープと変更前後）

**Files:**
- Create: `gpui/crates/track-core/src/ops.rs`（`lib.rs` で `pub mod ops;`）
- Test: `gpui/crates/track-core/src/ops_tests.rs`

**Interfaces:**
- Consumes: Task 1 のテーブルと入れ子書き込み
- Produces:

```rust
pub enum Table { Client, Project, Tag, TagOnProject, TimeEntry, TagOnEntry, AiRegistration }
// 主キー列: AiRegistration は "requestId"、ほかは "id"
pub struct Scope { pub table: Table, pub column: &'static str, pub value: String }
pub struct Recorder { /* scopes と before */ }
impl Recorder {
    /// 変更前の行を読んで保存（既存行を変える・消すとき）
    pub fn watch(&mut self, db: &Db, table: Table, column: &'static str, value: &str) -> Result<()>;
    /// 新しく作った行（変更前は空）
    pub fn created(&mut self, table: Table, column: &'static str, value: &str);
}
#[derive(Serialize)] #[serde(rename_all = "camelCase")]
pub struct Operation { pub id: i64, pub created_at: DateTime<Utc>, pub source: String, pub summary: String, pub count: i64, pub undone_at: Option<DateTime<Utc>> }
impl Db {
    /// BEGIN IMMEDIATE → f → 全スコープの変更後を読む → AiOperation を挿入 → 間引き → COMMIT。f が Err なら ROLLBACK
    pub fn with_operation<T>(&mut self, source: &str, f: impl FnOnce(&mut Db, &mut Recorder) -> Result<(T, String, i64)>) -> Result<(T, Operation)>;
    pub fn operations(&self, limit: i64) -> Result<Vec<Operation>>;           // 新しい順
    pub fn latest_operation_id(&self) -> Result<i64>;                          // 無ければ 0
    pub fn operations_after(&self, id: i64) -> Result<Vec<Operation>>;        // id より新しいもの、新しい順
}
```

`changes` 列は `[{ "table", "column", "value", "before": [行], "after": [行] }]`。行は `SELECT *` の列名→値（INTEGER/REAL/TEXT/NULL）の JSON オブジェクトで、主キー順に並べる。`(T, String, i64)` の String は summary、i64 は count。

- [ ] **Step 1: 失敗するテストを書く**
  - `operation_records_before_and_after_rows`: 工数 1 件を `with_operation` 内で `watch(TimeEntry,"id",id)`・`watch(TagOnEntry,"entryId",id)` してから `update_entry` でタイトル変更 → `operations(10)` が 1 件、summary・count・source が渡した値。`changes` の TimeEntry の before.title が旧、after.title が新
  - `failed_operation_changes_nothing_and_records_nothing`: f の中で 1 件更新した後に `Err` を返す → 工数は元のまま、`operations(10)` は空
  - `operations_are_pruned_to_100_and_30_days`: 101 回記録 → 100 件。`createdAt` を 31 日前に書き換えた行は次の記録で消える
  - `operations_after_returns_newer_only`
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-core ops_tests` / Expected: FAIL（`ops` が無い）
- [ ] **Step 3: 実装** — 上の Interfaces どおり。スコープは同じ (table, column, value) を重複させない
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test -p track-core` / Expected: PASS
- [ ] **Step 5: Commit** — `feat(core): AI 操作の記録`

### Task 3: 取り消し

**Files:**
- Modify: `gpui/crates/track-core/src/ops.rs`
- Test: `gpui/crates/track-core/src/ops_tests.rs`

**Interfaces:**
- Produces:

```rust
pub struct UndoResult { pub operation: Operation }   // Serialize camelCase
impl Db {
    /// id 省略時は未取り消しの最新。BEGIN IMMEDIATE の中で undoneAt を確かめる
    pub fn undo_operation(&mut self, id: Option<i64>) -> Result<UndoResult>;
}
```

エラー: `nothing_to_undo`（対象なし）、`not_found`（id が無い・間引き済み）、`already_undone`、`undo_conflict`（details: `{ "changed": [{ "table", "key" }] }`）。

復元手順（全スコープの一致を確かめてから）: `PRAGMA defer_foreign_keys = ON`。主キーごとに「現在にあって変更前に無い行は DELETE」「両方にある行は変更前の値で UPDATE」「変更前にだけある行は INSERT」。DELETE は子から（AiRegistration, TagOnEntry, TagOnProject, TimeEntry, Project, Tag, Client）、INSERT は親から（逆順）。最後に `undoneAt` を入れる。UPDATE を使うのは、親行を消して入れ直すと ON DELETE CASCADE / SET NULL が子を壊すため。

- [ ] **Step 1: 失敗するテストを書く**
  - `undo_restores_updated_entry_and_tags`: タイトルとタグを変えた操作を取り消す → タイトル・タグが元どおり、`operations` で `undone_at` が入る
  - `undo_of_ai_create_removes_entry_and_registration`: `create_ai_entry` を `with_operation` 内で行い `created(TimeEntry/TagOnEntry/AiRegistration)` → 取り消すと工数も `AiRegistration` も無く、`ai_coverage(source, session)` が空
  - `undo_of_tag_delete_restores_links`: タグ付きの工数とプロジェクトがある状態でタグ削除（`watch(Tag,"id")`, `watch(TagOnEntry,"tagId")`, `watch(TagOnProject,"tagId")`）→ 取り消すとタグと両方の付与が戻る
  - `undo_conflicts_when_row_changed_later`: 操作のあと `update_entry` で別のタイトルにする → `undo_conflict`、details の changed に TimeEntry とその id、工数は手で変えた値のまま（Review Focus 1）
  - `undo_twice_is_rejected`: 2 回目は `already_undone`（Review Focus 4）
  - `undo_without_id_takes_latest_not_undone`、`nothing_to_undo_when_all_undone`
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-core ops_tests` / Expected: FAIL
- [ ] **Step 3: 実装**
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test -p track-core` / Expected: PASS
- [ ] **Step 5: Commit** — `feat(core): AI 操作の取り消し`

### Task 4: 編集操作（core）

**Files:**
- Create: `gpui/crates/track-core/src/edit.rs`（`pub mod edit;`）
- Modify: `gpui/crates/track-core/src/repo/entries.rs`（検索）
- Test: `gpui/crates/track-core/src/edit_tests.rs`

**Interfaces:**
- Consumes: `with_operation`, `Recorder`, 既存の `update_entry` / `delete_entry` / `create_*` / `update_*` / `delete_tag`
- Produces（すべて 1 操作を記録し `(結果, Operation)` を返す。summary は日本語の 1 行）:

```rust
pub struct EntrySearch { pub project_id: Option<String> /* "none" = プロジェクトなし */, pub query: Option<String>, pub from: Option<DateTime<Utc>>, pub to: Option<DateTime<Utc>> }
impl Db {
    pub fn search_entries(&self, q: &EntrySearch, limit: i64) -> Result<Vec<TimeEntry>>; // 新しい順、上限 1000
    pub fn edit_entries(&mut self, source: &str, ids: &[String], patch: EntryPatch) -> Result<(Vec<TimeEntry>, Operation)>;
    pub fn remove_entries(&mut self, source: &str, ids: &[String]) -> Result<((), Operation)>;
    /// replay（同じ要求 ID の再送）では操作を記録せず None
    pub fn record_ai_entry(&mut self, source: &str, input: NewEntry, reg: AiRegistrationInput) -> Result<(TimeEntry, bool, Option<Operation>)>;
    pub fn op_create_client(&mut self, source: &str, name: &str) -> Result<(Client, Operation)>;
    pub fn op_update_client(&mut self, source: &str, id: &str, patch: ClientPatch) -> Result<(Client, Operation)>;
    pub fn op_create_project(&mut self, source: &str, client_id: &str, name: &str, color: &str, tag_ids: &[String]) -> Result<(Project, Operation)>;
    pub fn op_update_project(&mut self, source: &str, id: &str, patch: ProjectPatch) -> Result<(Project, Operation)>;
    pub fn op_create_tag(&mut self, source: &str, name: &str, color: &str) -> Result<(Tag, Operation)>;
    pub fn op_update_tag(&mut self, source: &str, id: &str, patch: TagPatch) -> Result<(Tag, Operation)>;
    pub fn op_delete_tag(&mut self, source: &str, id: &str) -> Result<((), Operation)>;
}
pub const PROJECT_COLORS: [&str; 10]; // track-app settings/projects.rs の COLORS をここへ移し、app 側はこれを使う
```

スコープ: 工数は `TimeEntry.id`・`TagOnEntry.entryId`。削除では加えて削除前に `entryId` が一致した `AiRegistration` を `requestId` で watch（SET NULL を戻すため）。クライアント更新は `Client.id` と `Project.clientId`。プロジェクトは `Project.id`・`TagOnProject.projectId`。タグ削除は Task 3 のテストと同じ 3 つ。`edit_entries` で `start`/`end` を変えるのは ID が 1 件のときだけ（複数なら `invalid_input`）。ID は重複を除く。空なら `invalid_input`。

- [ ] **Step 1: 失敗するテストを書く**
  - `edit_entries_moves_all_to_another_project_and_undo_restores`: 3 件を別プロジェクトへ → 3 件とも変わる、count = 3、取り消すと元のプロジェクト
  - `edit_entries_with_unknown_id_changes_nothing`: 既存 2 件 + 存在しない ID → `not_found`、2 件は元のまま、操作は記録されない（Review Focus 2）
  - `remove_entries_and_undo_restores_registration_link`: AI 登録済みの工数を削除 → 取り消すと工数が戻り、`AiRegistration.entryId` もその工数を指す
  - `archiving_client_archives_projects_and_undo_unarchives_them`（Review Focus 3）
  - `search_by_project_and_literal_query`: `--query` に `100%` を渡すと `100%完了` だけに当たり `1000完了` には当たらない（Review Focus 5）。`project_id = "none"` はプロジェクトなしだけ
  - `create_project_records_operation_and_undo_removes_it`
  - `record_ai_entry_replay_records_no_operation`
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-core edit_tests` / Expected: FAIL
- [ ] **Step 3: 実装**（LIKE は `ESCAPE '\'` で `%` `_` `\` をエスケープ、title と note を対象）
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test -p track-core && cargo test -p track-app` / Expected: PASS
- [ ] **Step 5: Commit** — `feat(core): AI からの編集操作`

### Task 5: CLI コマンド

**Files:**
- Modify: `gpui/crates/track-cli/src/main.rs`（必要なら `src/edit.rs` に分ける）
- Test: `gpui/crates/track-cli/tests/edit.rs`（既存テストの `Env` を共有モジュールに出してよい）

**Interfaces:**
- Consumes: Task 4 の `Db` メソッド、Task 3 の `undo_operation`、Task 2 の `operations`
- Produces（usage にも載せる）:

```
track-cli search [--project-id <ID>|none] [--query <語>] [--from <ISO>] [--to <ISO>] [--limit <N>]   # 条件 1 つ以上必須
track-cli update --id <ID> [--id ...] [--project-id <ID>|--no-project] [--title <T>] [--note <N>|--clear-note]
                 [--tag-id <ID> ...|--clear-tags] [--start <ISO> --end <ISO>] [--break-minutes <分>] [--confirmed] [--source codex|claude]
track-cli delete --id <ID> [--id ...] --confirmed [--source ...]
track-cli clients | tags | projects [--all]
track-cli client create --name <名前> --confirmed | client update --id <ID> [--name <名前>] [--archived true|false]
track-cli project create --client-id <ID> --name <名前> [--color <#RRGGBB>] [--tag-id ...] --confirmed
track-cli project update --id <ID> [--client-id <ID>] [--name] [--color] [--archived true|false] [--tag-id ...|--clear-tags]
track-cli tag create --name <名前> [--color <#RRGGBB>] | tag update --id <ID> [--name] [--color] | tag delete --id <ID> --confirmed
track-cli history [--limit <N>]       # 既定 20
track-cli undo [--id <操作ID>]
```

書き込みの出力には `"operation": { id, summary, count, ... }` を含める。`--source` 省略時の source は `"cli"`。`--color` 省略時は `PROJECT_COLORS` から既存プロジェクト数で順に選ぶ（設定画面と同じ）。既存の `create` は `record_ai_entry` を使い、`--source` が無い場合も 1 操作として記録する（`with_operation` + 通常の `create_entry`）。

確認エラー: `confirmation_required`、message「N件の工数を変更します。一覧をユーザーに示して許可を得てから --confirmed を付けてください。」の形、details に `{ "count": N, "entries": [...] }`（削除・一括変更）または `{ "name": ... }`（作成）。件数は `update` の重複を除いた ID 数。

- [ ] **Step 1: 失敗するテストを書く**
  - `update_ten_entries_requires_confirmation`: 10 件 → exit 1、`confirmation_required`、何も変わらない。`--confirmed` 付きで成功し、出力に operation
  - `update_nine_entries_runs_without_confirmation`
  - `delete_requires_confirmation_even_for_one`
  - `project_create_requires_confirmation`、`client_create_requires_confirmation`、`tag_delete_requires_confirmation`
  - `undo_after_update_restores_and_history_shows_undone`
  - `undo_conflict_is_reported_with_changed_rows`
  - `search_requires_a_condition`
  - `create_records_an_operation_that_undo_removes`
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-cli` / Expected: FAIL
- [ ] **Step 3: 実装**
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test -p track-cli` / Expected: PASS
- [ ] **Step 5: Commit** — `feat(cli): 工数・プロジェクトの編集と取り消し`

### Task 6: AI 連携の文面

**Files:**
- Create: `gpui/crates/track-core/src/integrations_edit.md`（GPUI 版で付け足す節）
- Modify: `gpui/crates/track-core/src/integrations.rs`（`adapt` で付け足し、description を書き換え）
- Test: `gpui/crates/track-core/src/integrations_tests.rs`

**Interfaces:**
- Consumes: Task 5 のコマンド名と確認ルール

付け足す節「## 工数・プロジェクトの編集と取り消し」に書くこと:
- 頼まれたら `search` / `list` / `projects --all` / `clients` / `tags` で対象を集め、`update` / `delete` / `client` / `project` / `tag` で変える。CLI の `$TRACK_CLI` 呼び出しは記録手順と同じ書き方（dev では `adapt` の置換で `TRACK_DATA_DIR` が付く）
- 書き込みには `--source codex` または `--source claude` を付ける
- 確認ルール（10 件以上・削除・クライアント／プロジェクト作成・タグ削除）。`confirmation_required` が返ったら一覧（日付・時間帯・プロジェクト・タイトル）を示して許可を取り、許可後に `--confirmed`。利用者が明示的に「作って」「消して」と頼んだ場合はその依頼を許可として扱ってよい
- 記録時に**明らかに**登録済みプロジェクトに一致しない作業（会話に新しい案件・顧客名が出てくる等）は、「プロジェクトなし」に寄せる前に、作るかを確認する。クライアントは既存から候補を出し、無ければ作るかも一緒に聞く。根拠が弱いときは従来どおり「プロジェクトなし」
- 実行後は operation の summary と id を伝え、「戻して」と言われたら `undo`（id 指定が確実）。`undo_conflict` なら変わった行を伝え、勝手に上書きしない
- Track アプリの画面にも［元に戻す］が出ること

description（Codex の SKILL.md frontmatter と Claude のコマンド）に「登録済み工数の修正・付け替え・削除、クライアント・プロジェクト・タグの作成と変更、取り消し」を加える。

- [ ] **Step 1: 失敗するテストを書く** — `adapted_texts_include_editing_section`: Codex・Claude の両方の `adapt(.., dev=true)` 結果に `track-cli` の `update` と `undo` の説明、`--confirmed`、`TRACK_DATA_DIR="$HOME/.track-gpui"` 付きの呼び出しが含まれる。`adapt(.., dev=false)` には `.track-gpui` が含まれない
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-core integrations` / Expected: FAIL
- [ ] **Step 3: 実装**
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test -p track-core` / Expected: PASS
- [ ] **Step 5: Commit** — `feat(core): AI 連携の文面に編集と取り消しを追加`

### Task 7: 画面の［元に戻す］

**Files:**
- Modify: `gpui/crates/track-app/src/data/mod.rs`（`latest_operation_id`, `operations_after`, `undo_operation`）
- Modify: `gpui/crates/track-app/src/app.rs`（`AppState` に最後に見た操作 ID と表示中の操作、`refresh_all` の後で新しい操作を取る。`TrackApp::render` で本体の上にバナー）
- Create: `gpui/crates/track-app/src/ui/ai_operation_banner.rs`
- Modify: `gpui/crates/track-app/src/settings/projects.rs`（`COLORS` を `track_core::edit::PROJECT_COLORS` に）
- Test: `gpui/crates/track-app/src/app_tests.rs`

**Interfaces:**
- Consumes: Task 2/3 の `latest_operation_id`, `operations_after`, `undo_operation`
- Produces: バナーの debug selector `ai-op-banner`, `ai-op-undo`, `ai-op-close`, `ai-op-error`

挙動: 起動時（`load`）に `latest_operation_id` を覚える（それ以前は出さない）。`refresh_all` の完了後に `operations_after(seen)` を読み、未取り消しの最新があればそれを表示し `seen` を最大 ID に進める。文言「AI が N 件変更しました：{summary}」と［元に戻す］［×］。［元に戻す］は `undo_operation(Some(id))` → 成功でバナーを消し `refresh_all`。失敗（`undo_conflict` / `already_undone`）は message をバナー内に出す。［×］か次の操作が来るまで出しておく。

- [ ] **Step 1: 失敗するテストを書く**
  - `ai_operation_shows_banner_and_undo_restores`: アプリを開いた後、別接続（テスト用 `Db::open`）で `edit_entries` → `refresh_all` を待つ → `ai-op-banner` が出る → `ai-op-undo` をクリック → DB の工数が元に戻り、バナーが消える
  - `operations_before_launch_are_not_shown`
  - `undo_conflict_shows_error_in_banner`
- [ ] **Step 2: 失敗を確認** — Run: `cd gpui && cargo test -p track-app app_tests` / Expected: FAIL
- [ ] **Step 3: 実装**
- [ ] **Step 4: 通過を確認** — Run: `cd gpui && cargo test --workspace && cargo clippy --workspace --all-targets` / Expected: PASS、警告なし
- [ ] **Step 5: Commit** — `feat(app): AI の操作を画面から取り消す`

### Task 8: ドキュメントと実地確認

**Files:**
- Modify: `gpui/README.md`（「AI 連携」に編集・取り消しのコマンドと確認ルール、画面の［元に戻す］）
- Modify: `gpui/NOTES.md`（この機能の所感と残り）

- [ ] **Step 1: コピー DB で CLI を実地確認** — `TRACK_DATA_DIR="$HOME/.track-gpui"` で `search --project-id <実在ID>` → `update`（1 件、タイトル変更）→ `history` → `undo` を実行し、値が戻ることを確かめる（コピー DB だけ。本物の `~/.track` には触れない）
- [ ] **Step 2: アプリを起動したまま CLI で `update` → 1 秒以内にバナーが出ることを、アプリのウインドウだけの撮影（`scripts/winshot.sh`）で確かめる。撮影は commit しない
- [ ] **Step 3: ドキュメントを更新**
- [ ] **Step 4: 全テスト** — Run: `cd gpui && cargo test --workspace` / Expected: PASS
- [ ] **Step 5: Commit** — `docs(gpui): AI からの編集と取り消し`
