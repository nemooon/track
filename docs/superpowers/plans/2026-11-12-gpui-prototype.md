# Track GPUI 試作 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Track のカレンダー画面とレポート画面を、既存の Bun API につなぐ GPUI（Rust）製デスクトップアプリとして、今の React 版と同等に再実装する。

**Architecture:** `gpui/` に独立した Cargo バイナリを置く。純粋なロジック（日付・配置・集計・コピー文字列・プロンプト・SSE/NDJSON の解析）は UI から切り離した `core` モジュール群に置いて単体テストし、UI は gpui-component の部品と GPUI の `Entity` で組む。HTTP は専用 tokio ランタイム上の reqwest で行い、GPUI の非同期タスクから待つ。

**Tech Stack:** Rust 1.97+、`gpui-pre =0.3.12`、`gpui-pre-platform =0.3.12`（`font-kit`, `runtime_shaders`）、`gpui-component =0.9.1`、reqwest + tokio、serde / serde_json、chrono、（祝日クレートは Task 3 で選定）

**Spec:** `docs/superpowers/specs/2026-11-12-gpui-prototype-design.md`

## Global Constraints

- 既存の React / Tauri / Bun のコード（`src/`, `src-tauri/`, `database/`, ルートの `package.json`）は変更しない。変更は `gpui/` と `docs/` のみ
- 依存バージョンは `=` で固定する。ビルド不能・部品が壊れているときのみ変更可、その場合は `gpui/NOTES.md` に記録
- API の接続先は既定 `http://127.0.0.1:8787/api`。環境変数 `TRACK_PORT` があればそのポート
- 動作確認は `TRACK_DATA_DIR=~/.track-gpui npm run local` で起動したコピー DB に対して行う。`~/.track/` には書き込まない
- 試作アプリの保存ファイルは `~/.track-gpui/gpui-prototype.json`
- 週は日曜始まり、時刻はローカルタイムで扱い API には RFC3339（UTC）で送る
- スナップ 15 分、表示 0:00–24:00、上下バッファ 32px、時刻欄 60px、ズーム `hourPx` = 48 / 96 / 192（初期 index 1）
- 画面の文言は今の React 版と同じ日本語（元コードからそのまま写す）
- API のエラー本文のキーは `error`
- Xcode 本体が必要と判明した時点で作業を止めてユーザーに頼む

## Review Focus

1. **0:00 で終わる工数（翌日 00:00 終了）**: その日の 24:00 まで描かれ、日の合計にも入り、翌日の列には出ない → Task 3 のテスト `entry_ending_at_midnight_belongs_to_start_day`
2. **サーバーが保存を拒否（400 `entry_must_be_same_day` など）**: 楽観的に動かしたブロックが元に戻り、「更新に失敗しました」が出る。エラー本文 `{"error": "..."}` を読める → Task 2 のテスト `api_error_reads_error_key`、Task 6 のテスト `failed_update_rolls_back`
3. **API サーバーが起動していない／途中で落ちる**: アプリは落ちずに「サーバーに接続できません」と再試行ボタンを出し、SSE は再接続する → Task 2 のテスト `sse_parser_handles_partial_chunks`、Task 4 の手動確認
4. **ドラッグが列や時間の端を越える**: 移動は 0:00–24:00 に収まり長さを保つ、最終列より右に出ても最終列に留まる、伸縮は最短 15 分 → Task 6 のテスト `move_clamps_to_day_bounds_and_columns`、`resize_keeps_min_15`
5. **日本語入力中の Enter/↑↓**: タイトル候補の選択・作成は IME 変換中には発火しない → Task 7 の手動確認項目（gpui-component の Input が変換中フラグを持つかを確認し、持たなければ NOTES.md に記録）

---

## ファイル構成

```
gpui/
  Cargo.toml / README.md / NOTES.md
  scripts/prepare-db.sh         コピー DB の準備
  src/main.rs                   起動・メニュー・キー割り当て
  src/app.rs                    ルートビュー（ヘッダー＋表示中の画面）
  src/actions.rs                アクション定義
  src/persist.rs                ~/.track-gpui/gpui-prototype.json の読み書き
  src/api/{mod.rs,types.rs,client.rs,sse.rs,ndjson.rs}
  src/core/{time.rs,layout.rs,geometry.rs,holidays.rs,interaction.rs}
  src/core/report/{group.rs,copy.rs,aggregation.rs,cache.rs,weekly.rs}
  src/ui/{date_nav.rs,date_picker.rs,theme.rs}
  src/calendar/{mod.rs,grid.rs,entry_block.rs,picker.rs,edit_dialog.rs,toolbar.rs}
  src/reports/{mod.rs,filters.rs,summary.rs,details.rs,custom_output.rs,weekly_report.rs}
```

`core/` は gpui に依存しない（単体テスト対象）。`calendar/` `reports/` `ui/` は gpui に依存する。

---

### Task 1: 骨組みとビルド可否の確認

**Files:**
- Create: `gpui/Cargo.toml`, `gpui/src/main.rs`, `gpui/README.md`, `gpui/scripts/prepare-db.sh`, `gpui/.gitignore`（`target/`）

**Interfaces:**
- Produces: `cargo run` でウインドウ（1200×800、タイトル「Track (GPUI)」）が開くバイナリ `track-gpui`

- [ ] **Step 1: `Cargo.toml` を書く**
  `[dependencies]` に `gpui = { package = "gpui-pre", version = "=0.3.12" }`、`gpui_platform = { package = "gpui-pre-platform", version = "=0.3.12", features = ["font-kit", "runtime_shaders"] }`、`gpui-component = "=0.9.1"`。`[profile.dev.package."*"] opt-level = 2`（依存のビルドを速くするため）。gpui-component が要求する gpui の指定方法が上と違えば、gpui-component の `Cargo.toml` に合わせる。
- [ ] **Step 2: `main.rs` で gpui-component を初期化しウインドウを開く**
  ルートは gpui-component の `Root` で包む（ダイアログ・通知・ポップオーバーの土台になるため）。中身は「Track (GPUI)」と表示するだけ。
- [ ] **Step 3: ビルドして起動する**
  Run: `cd gpui && cargo run`
  Expected: ウインドウが開き文字が描画される。`unable to find utility "metal"` で失敗したら `runtime_shaders` が効いているか確認し、それでも駄目なら作業を止めてユーザーに Xcode 本体のインストールを依頼する。
- [ ] **Step 4: `scripts/prepare-db.sh` を書く**
  `mkdir -p ~/.track-gpui`、`sqlite3 ~/.track/track.db ".backup $HOME/.track-gpui/track.db"`、`cp ~/.track/config.json ~/.track-gpui/`。既に `~/.track-gpui/track.db` があれば `--force` なしでは上書きしない。
- [ ] **Step 5: 実行して API が応答することを確認**
  Run: `gpui/scripts/prepare-db.sh && TRACK_DATA_DIR=~/.track-gpui npm run local`（別端末で）→ `curl -s http://127.0.0.1:8787/api/projects | head -c 200`
  Expected: プロジェクトの JSON が返る
- [ ] **Step 6: README に起動手順（準備スクリプト → API 起動 → `cargo run`）を書いてコミット**
  `git commit -m "feat(gpui): 試作の骨組みを追加"`

### Task 2: API クライアント

**Files:**
- Create: `gpui/src/api/{mod.rs,types.rs,client.rs,sse.rs,ndjson.rs}`

**Interfaces:**
- Produces:
  - `types.rs`: `TimeEntry`, `Project`, `Client`, `Tag`, `Settings`, `AppConfig`(`ai_provider` のみ), `ReportRow`, `ReportResponse`, `ReportEntry`, `ReportEntriesResponse`, `ReportCopyFormat`, `ReportCopyColumn`（`kind` でタグ付き enum: `Field{id,field,label?,duration_format?}` / `Ai{id,label,prompt}` / `Blank{id,label?}`）, `ReportDurationFormat`, `NewEntry`, `EntryPatch`, `ReportQuery{range,anchor,group_by,client_ids,project_ids,tag_ids}`。形と名前は `src/shared/types.ts` を serde の `rename_all = "camelCase"` で写す
  - `client.rs`: `ApiClient::new(base: String) -> ApiClient`（`Clone`、内部に `Arc<tokio::runtime::Runtime>` と `reqwest::Client`）。メソッドはすべて `fn x(&self, ..) -> impl Future<Output = Result<T, ApiError>> + Send + 'static`（tokio 上で実行し結果を返す）: `entries(from, to)`, `entry_titles(project_id: Option<&str>)`, `create_entry(NewEntry)`, `update_entry(id, EntryPatch)`, `delete_entry(id)`, `projects()`, `clients()`, `tags()`, `settings()`, `config()`, `report(ReportQuery)`, `report_entries(ReportQuery)`
  - `ApiError { status: Option<u16>, message: String }`
  - `sse.rs`: `SseParser::push(&mut self, chunk: &str) -> Vec<SseEvent{event: String, data: String}>`、`ApiClient::subscribe_events() -> async_channel::Receiver<SseEvent>`（切断時は 1 秒待って再接続）
  - `ndjson.rs`: `NdjsonParser::push(&mut self, chunk: &str) -> Vec<AiStreamEvent>`、`AiStreamEvent::{Progress{kind,message}, Heartbeat, Result{text,provider}, Error{message}}`、`ApiClient::generate_stream(mode: &str, input: String) -> async_channel::Receiver<AiStreamEvent>`

- [ ] **Step 1: 失敗するテストを書く**（`types.rs`, `sse.rs`, `ndjson.rs`, `client.rs` 内の `#[cfg(test)]`）
  - `deserializes_time_entry`: `src/shared/types.ts` の形に沿ったサンプル JSON（project/client/tags 入り、`breakMinutes: 30`）が読める
  - `deserializes_copy_format_columns`: `{"kind":"ai","id":"a","label":"L","prompt":"P"}` と `{"kind":"blank","id":"b"}` が対応する variant になる
  - `api_error_reads_error_key`: `ApiError::from_body(400, r#"{"error":"entry_must_be_same_day"}"#).message == "entry_must_be_same_day"`、本文が JSON でなければ本文そのもの、空なら `"HTTP 400"`
  - `sse_parser_handles_partial_chunks`: `"event: entries.changed\nda"` → 空、続けて `"ta: {\"x\":1}\n\n"` → 1 件（event=`entries.changed`、data=`{"x":1}`）。`event` 行がなければ `message`
  - `ndjson_parser_splits_lines`: `"{\"type\":\"progress\",\"kind\":\"status\",\"message\":\"a\"}\n{\"type\":\"res"` → Progress 1 件、続けて `"ult\",\"text\":\"T\",\"provider\":\"codex\"}\n"` → Result 1 件
- [ ] **Step 2: `cd gpui && cargo test api::` が失敗することを確認**
- [ ] **Step 3: 実装する**
- [ ] **Step 4: `cargo test api::` が通ることを確認**
- [ ] **Step 5: 実 API で確認**: `main.rs` に一時的に `projects()` を呼んで件数を表示する処理を入れ、コピー DB の件数が出ることを確認したら消す
- [ ] **Step 6: コミット** `feat(gpui): API クライアントを追加`

### Task 3: 日付・配置・祝日のロジック

**Files:**
- Create: `gpui/src/core/{mod.rs,time.rs,geometry.rs,layout.rs,holidays.rs}`

**Interfaces:**
- Produces（`chrono::{NaiveDate, DateTime<Local>}` を使う）:
  - `time.rs`: `SNAP_MIN: i64 = 15`、`week_range(anchor: NaiveDate) -> (NaiveDate, NaiveDate)`（日曜始まり、終わりは翌週日曜＝排他）、`month_range(anchor) -> (NaiveDate, NaiveDate)`（終わりは翌月1日）、`DayCount::{One, Three, Workdays, Seven}`、`visible_days(anchor, DayCount, work_days: &[u32]) -> Vec<NaiveDate>`、`step_days(DayCount) -> i64`、`effective_day_count(requested: DayCount, window_width: f32) -> DayCount`（≤639→One、≤767 で Workdays/Seven→Three）、`range_label(from, to_exclusive, is_month: bool) -> String`（「M月d日」「M月d日 – d日」「M月d日 – M月d日」「M月」）、`format_duration_report(minutes: i64) -> String`（`ReportsPage.tsx:98-102`: 分>0 なら `Xh Ym`、それ以外 `Xh`。30 分は `0h 30m`）、`format_duration_calendar(minutes: i64) -> String`（`EntryBlock.tsx:60`: 1 時間以上は `Xh`／`Xh Ym`、未満は `Ym`。30 分は `30m`）、`weekday_ja(d) -> &'static str`（日月火水木金土）
  - `geometry.rs`: `BUFFER_PX = 32.`、`GUTTER_PX = 60.`、`ZOOM_LEVELS = [48., 96., 192.]`、`y_to_minutes(y: f32, hour_px: f32) -> i64`（15分に丸め 0..=1440 に収める）、`minutes_to_y(min: i64, hour_px) -> f32`、`minutes_of_day(dt: DateTime<Local>, day: NaiveDate) -> i64`（`day` の翌日 0:00 なら 1440）、`at_minutes(day, min) -> DateTime<Local>`
  - `layout.rs`: `layout_blocks<T>(items: &[T], span: impl Fn(&T) -> (i64, i64)) -> Vec<Laid{index: usize, lane: usize, lane_count: usize}>`（`src/client/components/calendar/layout.ts` と同じアルゴリズム）、`entries_for_day(entries: &[TimeEntry], day) -> Vec<&TimeEntry>`（開始日で振り分け）、`day_total_minutes(entries, day) -> i64`（`end-start-breakMinutes`）
  - `holidays.rs`: `holiday_name(d: NaiveDate) -> Option<&'static str>`

- [ ] **Step 1: 失敗するテストを書く**
  - `week_range_starts_sunday`: 2026-11-12（木）→ (2026-11-08, 2026-11-15)
  - `month_range`: 2026-02-10 → (2026-02-01, 2026-03-01)
  - `workdays_view_filters_by_work_days`: anchor 2026-11-12、work_days `[1,2,3,4,5]` → 11/9〜11/13 の 5 日
  - `effective_day_count_breakpoints`: (Seven, 639.)→One、(Seven, 700.)→Three、(One, 700.)→One、(Seven, 768.)→Seven
  - `range_labels`: (11/8, 11/15)→「11月8日 – 14日」、(10/26, 11/2)→「10月26日 – 11月1日」、(11/12, 11/13)→「11月12日」、月→「11月」
  - `y_to_minutes_snaps_and_clamps`: hour_px 96 で y=50 →30、y=-10→0、y=99999→1440
  - `entry_ending_at_midnight_belongs_to_start_day`: 11/12 23:00–11/13 00:00 は 11/12 にだけ入り、`minutes_of_day(end, 11/12) == 1440`、11/12 の合計 60
  - `layout_matches_ts`: A 9:00–10:00、B 9:30–11:00、C 10:00–10:30、D 12:00–13:00 → A lane0, B lane1, C lane0, 3 つとも lane_count 2、D は lane0・lane_count 1
  - `duration_formats`: report(30)=`0h 30m`、report(120)=`2h`、calendar(30)=`30m`、calendar(90)=`1h 30m`
  - `holiday_known_dates`: 2026-11-03 → Some("文化の日")、2026-11-12 → None
- [ ] **Step 2: `cargo test core::` が失敗することを確認**
- [ ] **Step 3: 実装する**。祝日は crates.io で日本の祝日クレートを探し、2026–2027 年を正しく返すものを使う。なければ `@holiday-jp/holiday_jp` の `holidays.yml` 相当（2020–2030）を `holidays.rs` に表として埋め込む
- [ ] **Step 4: `cargo test core::` が通ることを確認**
- [ ] **Step 5: コミット** `feat(gpui): 日付と配置のロジックを追加`

### Task 4: アプリの外枠（ヘッダー・画面切替・メニュー・ショートカット・日付ナビ・保存）

**Files:**
- Create: `gpui/src/{app.rs,actions.rs,persist.rs}`, `gpui/src/ui/{theme.rs,date_nav.rs,date_picker.rs}`
- Modify: `gpui/src/main.rs`

**Interfaces:**
- Consumes: Task 2 `ApiClient`、Task 3 `time::*`
- Produces:
  - `actions.rs`: `ShowCalendar`, `ShowReports`, `PrevPeriod`, `NextPeriod`, `Today`, `ZoomIn`, `ZoomOut`（GPUI `actions!`）
  - `app.rs`: `TrackApp { api: ApiClient, page: Page, calendar: Entity<CalendarView>, reports: Entity<ReportsView>, store: Entity<PersistStore> }`、`Page::{Calendar, Reports}`。`PrevPeriod` などは表示中の画面へ転送する。最初の読み込みで `projects/tags/settings/clients/config` を取り、`AppData`（`Entity`）として両画面に渡す。取得失敗時は「サーバーに接続できません」と「再試行」ボタンを全面に出す
  - `persist.rs`: `PersistState { zoom_index: usize, reports_expanded: HashMap<String, Vec<String>>, group_same_titles: bool, summary_collapsed: bool, output_cache: Vec<CachedOutput> }`（serde、ファイル無し・壊れていれば既定値）、`PersistStore::update(&mut self, f)` で即保存
  - `date_nav.rs`: `DateNav::new(range: (NaiveDate, NaiveDate), is_month: bool, includes_today: bool, on_prev, on_next, on_today, on_pick: Fn(NaiveDate))`（‹ ラベル › 今日）
  - `date_picker.rs`: `DatePickerMode::{Day, Week, Month}` のポップオーバー
  - `theme.rs`: 色定数（ヘッダー濃緑、アンバー、赤線など）を `src/client/index.css` から写す
  - この Task の時点の `CalendarView` / `ReportsView` は日付ナビだけを持つ空の画面でよい（後続 Task で中身を作る）

- [ ] **Step 1: `persist.rs` のテストを書く**: `persist_roundtrip`（一時ディレクトリに保存→読み戻しで同値）、`persist_corrupt_file_falls_back_to_default`
- [ ] **Step 2: 失敗確認 → 実装 → `cargo test persist::` 通過**
- [ ] **Step 3: ヘッダー（44px、タブ「カレンダー」「レポート」）、画面切替、`cx.set_menus` の「表示」メニュー（カレンダー ⌘1／レポート ⌘2／前の期間 ⌘[／次の期間 ⌘]／今日 ⌘T／拡大 ⌘+／縮小 ⌘-）、`cx.bind_keys` を実装する**
- [ ] **Step 4: 日付ナビと日付ピッカーを両画面のツールバーに置く**
- [ ] **Step 5: 手動確認**: `cargo run` → ⌘1/⌘2 で切替、⌘[ ⌘] ⌘T でラベルが変わる、ピッカーの 3 モード、メニューからも同じ動作。API を止めて起動すると接続エラー画面、API を起動して「再試行」で復帰
- [ ] **Step 6: コミット** `feat(gpui): ヘッダー・メニュー・日付ナビを追加`

### Task 5: カレンダーの表示

**Files:**
- Create: `gpui/src/calendar/{mod.rs,grid.rs,entry_block.rs,toolbar.rs}`

**Interfaces:**
- Consumes: Task 3 全部、Task 4 `AppData`, `PersistStore`, `DateNav`
- Produces: `CalendarView { anchor: NaiveDate, day_count: DayCount, entries: Vec<TimeEntry>, selected: Option<String>, drag: DragState(Task 6), .. }`、`CalendarView::reload(&mut self, cx)`（表示範囲の `/entries` を取り直す）

- [ ] **Step 1: ツールバー**: 日付ナビ、表示日数ボタン（1日／3日／稼働日／1週間、幅で非表示）、ズーム −/+（端で無効）
- [ ] **Step 2: 時間グリッド**: 時刻欄、30 分破線・1 時間実線、上下 32px の斜線バッファ、勤務時間の網掛け（稼働日かつ祝日でない日だけ）、日付ヘッダー（丸・曜日・祝日バッジ・その日の合計・今日アンバー）、時刻欄と列のスクロール同期、列見出しの右余白をスクロールバー幅に合わせる。描画は `div` で組めるところは `div`、線・斜線は `canvas`
- [ ] **Step 3: ブロック**: `layout_blocks` で配置、`EntryBlock.tsx` と同じ寸法（左 `lane*100%/count + 2px`、幅 `100%/count - 6px`、上 +2、高さ -5、最小 14）、色は `index.css:66-101` の color-mix をプロジェクト色と白の線形補間で再現、表示内容は spec どおり
- [ ] **Step 4: 現在時刻の赤線（60 秒ごと）、ホバー線と時刻表示、初回スクロール位置（定時開始と現在の早いほう −1 時間）**
- [ ] **Step 5: 手動確認**: コピー DB の今週を React 版（`npm run dev`、同じ API）と並べ、ブロック位置・色・合計・祝日・網掛けが一致することを目視確認。スクリーンショットを `gpui/NOTES.md` 用に保存
- [ ] **Step 6: コミット** `feat(gpui): カレンダー表示を追加`

### Task 6: カレンダーの操作（ドラッグ・キーボード・楽観的更新・取り消し）

**Files:**
- Create: `gpui/src/core/interaction.rs`
- Modify: `gpui/src/calendar/{mod.rs,grid.rs,entry_block.rs}`

**Interfaces:**
- Consumes: Task 3 `geometry`、Task 2 `create_entry/update_entry/delete_entry`
- Produces:
  - `interaction.rs`（gpui 非依存）: `DragState::{Idle, Creating{day, anchor_min, current_min}, Picking{day, start_min, end_min}, Moving{id, day, offset_min, duration, start_min}, Resizing{id, day, edge: Edge, start_min, end_min}}`、`Edge::{Top, Bottom}`、関数 `begin_create(day, min)`, `update_create(state, min)`, `finish_create(state) -> Option<Picking>`（15 分未満は None）、`update_move(state, day_index: isize, day_count: usize, min) -> (day_index, start_min, end_min)`（長さ保持、0..=1440 に収める、列は 0..day_count-1 に収める）、`update_resize(state, min)`（最短 15）、`nudge(start, end, delta, shift: bool) -> (start, end)`（同日内）
  - `CalendarView::apply_optimistic(id, patch)`、`rollback(snapshot)`

- [ ] **Step 1: 失敗するテストを書く**（`interaction.rs`）
  - `create_under_15_minutes_is_cancelled`: 600→605 で None、600→615 で Picking(600,615)。上向きドラッグ（600→540）は (540,600)
  - `move_clamps_to_day_bounds_and_columns`: 長さ 60 を 1430 から始めると (1380,1440)、day_index 9 / day_count 7 → 6、-1 → 0
  - `resize_keeps_min_15`: Bottom を start と同じ位置に → end = start+15。Top を end より下に → start = end-15
  - `nudge_stays_within_day`: (0,60) を -15 → (0,60)、Shift で (600,615) を -15 → (600,615)
  - `failed_update_rolls_back`: `CalendarView` から切り出した `EntryCache`（`Vec<TimeEntry>` のラッパー、`interaction.rs` に置く）で `apply(id, start, end)` → `rollback(snapshot)` すると元に戻る
- [ ] **Step 2: `cargo test core::interaction` 失敗確認 → 実装 → 通過**
- [ ] **Step 3: マウス操作を配線する**: 空き部分のドラッグで作成（破線の仮ブロック）、ブロックのドラッグで移動（日またぎ、ウインドウ外でも追従）、上下 6px つまみで伸縮、クリック選択・空き部分で解除、ダブルクリックで編集ダイアログ（Task 7 まではログ出力でよい）。操作中はホバー線が動かしている端に追従
- [ ] **Step 4: 保存**: 変化があれば `update_entry`（楽観的、失敗で戻して通知「更新に失敗しました」、成功で「保存しました」）。削除は Delete/Backspace で即削除、通知「削除しました」＋「元に戻す」で同じ内容を `create_entry`。作成失敗は「記録の作成に失敗しました」。通知は gpui-component の Notification
- [ ] **Step 5: キーボード**: Esc 解除、↑↓ 15 分移動、Shift+↑↓ 終了伸縮（入力欄にフォーカスがあるときは無効）
- [ ] **Step 6: 手動確認**: コピー DB で作成・移動（日またぎ）・伸縮・削除→元に戻す・キー操作。0:00 終了の工数を作って表示確認。API を止めた状態で移動して元に戻ることを確認
- [ ] **Step 7: コミット** `feat(gpui): カレンダー操作を追加`

### Task 7: プロジェクト選択ポップオーバーと編集ダイアログ

**Files:**
- Create: `gpui/src/calendar/{picker.rs,edit_dialog.rs}`
- Modify: `gpui/src/calendar/mod.rs`

**Interfaces:**
- Consumes: Task 6 `DragState::Picking`、Task 2 `entry_titles`、`AppData`
- Produces: `ProjectPicker::new(start, end, data, on_submit: Fn(NewEntry))`、`ProjectSelect`（編集ダイアログ用の検索つき選択）、`EditDialog::open(entry, data, cx)`、純粋関数 `filter_projects(projects, query) -> Vec<(client_name, Vec<&Project>)>`、`title_suggestions(titles, input) -> Vec<String>`（前方一致・大小無視・完全一致除外・最大 8）、`merged_tag_ids(selected, locked) -> Vec<String>`

- [ ] **Step 1: 純粋関数のテストを書く**: `filter_projects_matches_client_or_project_name`（「acme」でクライアント名一致のプロジェクトが全部出る）、`title_suggestions_prefix_limit_8`（候補 10 件・入力「定例」→ 8 件、完全一致「定例」は除外）、`merged_tag_ids_unions_locked`
- [ ] **Step 2: 失敗確認 → 実装 → 通過**
- [ ] **Step 3: ピッカーを作る**: 幅 280、仮ブロックの横に固定し画面内に収める、手順 1（検索・クライアント別・プロジェクトなし）、手順 2（戻る・タイトル＋候補 ↑↓ Enter Esc・タグ・メモ・「作成 (⌘↵)」）。外側クリック／Esc で閉じて仮ブロックを消す
- [ ] **Step 4: 編集ダイアログ「予定を編集」**: タイトル、開始／終了（15 分刻み、日付は元のまま）、休憩（0–600 に収める、15 刻み）、プロジェクト選択、タグ（固定タグあり）、メモ、削除／キャンセル／保存、⌘↵ 保存、Esc・背景で閉じる
- [ ] **Step 5: 手動確認**: 日本語でタイトルを入力し、変換確定前の Enter／↑↓ で候補が選ばれないこと（Review Focus 5）。駄目なら NOTES.md に記録
- [ ] **Step 6: コミット** `feat(gpui): プロジェクト選択と編集ダイアログを追加`

### Task 8: レポートのロジック移植

**Files:**
- Create: `gpui/src/core/report/{mod.rs,group.rs,copy.rs,aggregation.rs,cache.rs,weekly.rs}`

**Interfaces:**
- Consumes: Task 2 の型
- Produces（元 TS と同名を snake_case で）:
  - `group.rs`: `group_entries_by_title(&[ReportEntry]) -> Vec<GroupedEntry>`（`ReportRow.tsx:160-200`）
  - `copy.rs`: `report_copy_format_uses_ai`, `report_copy_column_header`, `format_duration_as(minutes, ReportDurationFormat) -> String`, `aggregation_column_value`, `build_aggregation_copy_text`（`src/client/lib/reportCopy.ts`）
  - `aggregation.rs`: `AiAggregationRow`, `build_report_aggregation_prompt`, `aggregate_ai_result`（`reportAggregation.ts`）
  - `cache.rs`: `output_cache_key(..) -> String`, `save_output(cache: &mut Vec<CachedOutput>, key, rows, now)`, `load_output(cache, key, now) -> Option<..>`（最大 8 件・7 日、`reportOutputCache.ts`）。保存先は `PersistState::output_cache`
  - `weekly.rs`: `build_weekly_report_prompt(..)`, `replace_main_work_section(ai_text, fixed) -> String`（`ReportsPage.tsx:144-259`）

- [ ] **Step 1: 既存 TS テストを Rust に写す**: `reportCopy.test.ts` の 9 件、`reportAggregation.test.ts` の 3 件、`reportOutputCache.test.ts` の 4 件を、同じ入力と期待値でテストにする（テスト名は日本語の説明を英語の snake_case にする）。加えて `group_same_titles_drops_mixed_project`（同タイトル・別プロジェクト 2 件 → project None、count 2、minutes 合計）と `weekly_prompt_fills_placeholders`（`{{期間}}` `{{合計時間}}` が置換される）
- [ ] **Step 2: `cargo test core::report` 失敗確認**
- [ ] **Step 3: 実装する**。キャッシュキーは TS と同じ値でなくてよい（Rust 版の中だけで使う）が、何を含めるかは同じにする
- [ ] **Step 4: `cargo test core::report` 通過**
- [ ] **Step 5: コミット** `feat(gpui): レポートのロジックを移植`

### Task 9: レポート画面（ツールバー・絞り込み・サマリー・明細）

**Files:**
- Create: `gpui/src/reports/{mod.rs,filters.rs,summary.rs,details.rs}`

**Interfaces:**
- Consumes: Task 2 `report`, `report_entries`、Task 4、Task 8 `group_entries_by_title`
- Produces: `ReportsView { range: Range, anchor, group_by: GroupBy, client_ids, project_ids, tag_ids, data: Option<ReportResponse>, expanded: HashSet<String>, children: HashMap<String, Vec<ReportEntry>>, .. }`、`ReportsView::reload(cx)`、`ReportsView::current_query() -> ReportQuery`（Task 10 が使う）

- [ ] **Step 1: ツールバー**: 日付ナビ（週 ±7 日／月 ±1 ヶ月）、週／月、クライアント／プロジェクト／タグ
- [ ] **Step 2: 絞り込み 3 種**（`FilterMultiSelect.tsx` と同じ表記・挙動）、クライアント変更で合わないプロジェクト選択を外す、「すべてクリア」
- [ ] **Step 3: サマリー**: ドーナツ（内径 62%・外径 95%・間隔 2・角丸 4・白い縁 2・項目ごとのグラデーション・ツールチップ・中央に Total と合計）、凡例（色・名前・時間・割合・横棒）、折りたたみ時の積み上げ棒、色がない行は 10 色パレット（`ReportsPage.tsx:93-96`）。gpui-component の円グラフでグラデーションや角丸が出せなければ `canvas` で自作する。広い画面では左 300px、狭いと上に積む
- [ ] **Step 4: 明細**: 固定見出し・合計行・本文スクロールの表、グループ行の展開で `report_entries` を読む（パスキー `kind:key` を `>` でつなぐ）、全展開／全折りたたみ（全展開中はデータ更新で新しい行も展開）、「まとめる」、空表示・読み込み中。展開状態・まとめる・サマリー折りたたみは `PersistStore` に保存
- [ ] **Step 5: 手動確認**: コピー DB で React 版と並べ、週・月 × 3 グループ × 絞り込みで合計・割合・明細が一致することを確認
- [ ] **Step 6: コミット** `feat(gpui): レポート画面を追加`

### Task 10: カスタム出力と AI 週報

**Files:**
- Create: `gpui/src/reports/{custom_output.rs,weekly_report.rs}`
- Modify: `gpui/src/reports/mod.rs`

**Interfaces:**
- Consumes: Task 8 全部、Task 2 `generate_stream`, `report_entries`, `AppData.settings`, `AppData.config.ai_provider`、Task 9 `current_query`
- Produces: `CustomOutputDialog::open(query, rows, settings, cx)`、`WeeklyReportDialog::open(query, settings, cx)`

- [ ] **Step 1: カスタム出力ダイアログ**（`ReportsPage.tsx:1224-1395` に合わせる）: 形式選択（なければ注意書き）、AI 使用／なしバッジ、作成／再作成、AI 実行中の経過時間（m:ss）と「考えていること」「進捗」、エラー表示、結果の表、「保存済み結果を復元 · 日時」、結果をコピー（2 秒「コピーしました」）。プロバイダ名は codex→Codex、custom-command→カスタムコマンド、それ以外→Apple Intelligence
- [ ] **Step 2: AI 週報ダイアログ**: 週表示のときだけボタンを出す、テンプレート空・記録なしはエラー、生成中表示、複数行入力で Markdown を編集、閉じる／再生成／週報をコピー
- [ ] **Step 3: クリップボードは `cx.write_to_clipboard(ClipboardItem::new_string(..))`**
- [ ] **Step 4: 手動確認**: コピー DB で AI なし形式の出力・コピー（表計算に貼って列が揃う）、AI あり形式の出力（進捗が流れる）、ダイアログを閉じて開き直すと保存済み結果が復元される、週報の生成・編集・コピー
- [ ] **Step 5: コミット** `feat(gpui): カスタム出力と AI 週報を追加`

### Task 11: 変更の自動反映と所感メモ

**Files:**
- Modify: `gpui/src/app.rs`
- Create: `gpui/NOTES.md`

**Interfaces:**
- Consumes: Task 2 `subscribe_events`、`CalendarView::reload`、`ReportsView::reload`

- [ ] **Step 1: 起動時に `subscribe_events` を購読し、`track.connected` / `entries.changed` で両画面の `reload` と、展開中の明細の取り直しをする**
- [ ] **Step 2: 手動確認**: GPUI 版でカレンダーを開いたまま、React 版（または `curl -X POST`）で工数を作ると GPUI 版に出る。API を再起動しても再接続して反映が続く
- [ ] **Step 3: 計測**: `cargo build --release` のクリーンビルド時間と差分ビルド時間、release 版の起動から初回描画までの時間、メモリ使用量（アクティビティモニタまたは `ps -o rss`）、同じ操作時の React/Tauri 版のメモリ
- [ ] **Step 4: `gpui/NOTES.md` を書く**: 作りやすかった点・苦労した点・再現できなかった点と理由・依存を変えた場合はその内容・計測値・採用する場合に必要な作業（Rust でのデータ層の作り直し、配布、設定画面など）・Task 5/9 のスクリーンショット比較
- [ ] **Step 5: `cd gpui && cargo test && cargo clippy` が通ることを確認してコミット** `docs(gpui): 試作の所感メモを追加`
