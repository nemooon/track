# ドラッグ中の自動スクロールと合計フッター Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** カレンダーで記録をドラッグ中に上下の端で自動スクロールし、日ごとの合計をヘッダーから列の下のフッターへ移す。

**Architecture:** 端での速さは純粋関数 `geometry::edge_scroll_speed` で決める。`CalendarView` はドラッグ中の最後のマウス位置を覚え、端にいる間だけ 16ms ごとのタスクで `ScrollHandle` を進め、そのたびに `on_drag_move(最後の位置)` を呼び直す。フッターはスクロールの外、`calendar-scroll` の直後に置く。

**Tech Stack:** Rust, GPUI (gpui-pre 0.3.12)

**Spec:** docs/superpowers/specs/2026-11-18-drag-autoscroll-footer-design.md

## Global Constraints
- 端の幅 40px、速さは端に近いほど大きく、端の外では最大。最大 16px / 16ms。
- 自動スクロールは Creating・Moving・Resizing の間だけ。Idle・Picking では止める。
- 色は `palette(cx)` から。生の色は書かない。
- clippy 警告ゼロ（`--features track-core/real-data` 付きでも）。

## Review Focus
- スクロールが下限・上限に達したら止まり、オフセットが範囲外にならない。
- マウスを離した直後にタスクが残ってスクロールし続けない。
- 期間を移動（⌘]）してドラッグが取り消されたら止まる。
- フッターの列が時間の部分の列と横位置でそろう（左の目盛り幅を空ける）。

---

### Task 1: 端での速さ（純粋関数）

**Files:** Modify `crates/track-app/src/core/geometry.rs`（テストも同じファイルの tests）

- [ ] `pub const EDGE_ZONE_PX: f32 = 40.; pub const EDGE_MAX_STEP_PX: f32 = 16.;`
- [ ] `pub fn edge_scroll_speed(y: f32, top: f32, bottom: f32) -> f32` — 上の端なら負、下の端なら正、それ以外 0。端からの深さ d（0..=EDGE_ZONE_PX、外は EDGE_ZONE_PX）に比例して `EDGE_MAX_STEP_PX * d / EDGE_ZONE_PX`。
- [ ] テスト `edge_scroll_speed_grows_toward_the_edges`: 中央 0、top+40 で 0、top+20 で -8、top で -16、top-50 で -16、bottom-20 で 8、bottom+10 で 16。

### Task 2: ドラッグ中の自動スクロール

**Files:** Modify `crates/track-app/src/calendar/mod.rs`; Test `crates/track-app/src/calendar_tests.rs`

- [ ] テスト `dragging_to_the_bottom_edge_scrolls_and_carries_the_entry`: entry-a をつかみ、`calendar-scroll` の下端から 5px 上へ動かして止める。`advance_clock(500ms)` 後、スクロールのオフセットが下へ進み、`DragState::Moving` の start_min がマウスを止めたあとも増えている。離すと保存され、その後 `advance_clock` してもオフセットが変わらない。
- [ ] テスト `dragging_to_the_top_edge_scrolls_up`: 上端で同様にオフセットが上へ戻る（最初に下へスクロールしておく）。
- [ ] 実装: `last_drag_pos: Option<Point<Pixels>>`、`autoscroll: Option<Task<()>>`。`on_drag_move` で位置を記録し、速さが 0 でなければタスクを起こす。タスクは毎回 `step_autoscroll` を呼び、0 になるか drag が対象外ならタスクを外して終わる。オフセットは `-max_offset.y..=0` に収める。`on_drag_end` と `on_nav` でタスクを外す。

### Task 3: 合計フッター

**Files:** Modify `crates/track-app/src/calendar/mod.rs`; Test `crates/track-app/src/calendar_tests.rs`

- [ ] テスト `day_totals_sit_in_a_footer_under_their_columns`: `day-footer` が `calendar-scroll` の下にあり、`day-total-2026-11-12` が `entry-a` と横に重なり、"1h" を出している。ヘッダーに `day-total-*` がない。
- [ ] 実装: `render_day_footer(&days, cx)`。左に GUTTER_PX の空き、列ごとに border_l、右寄せで合計（0 なら空）。ヘッダーの合計を外す。
- [ ] `cargo test --workspace`、clippy 二通り、コピー DB で実画面を確認、commit・push。
