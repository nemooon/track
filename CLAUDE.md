# Track — 開発ガイド

## 技術スタック

- フロントエンド: Vite + React 19 + Tailwind CSS v4 + Zustand + TanStack Query
- ローカル API: Bun + Hono
- DB: SQLite + Prisma + libSQL アダプタ
- デスクトップ: Tauri 2（Bun コンパイル済み sidecar を同梱）

## コード配置

- `src/client/` — React SPA。ブラウザ固有のコードはここに置く
- `src/server/` — Bun API。`routes/`、`db/`、`fixtures/` に分ける
- `src/shared/` — client/server の両方から使う型、日付処理、Zod スキーマ
- `database/migrations/` — 起動時とリストア後に適用する手書き SQL
- `database/schema.prisma` — Prisma スキーマ
- `src-tauri/` — Tauri のRustコード、バンドル設定、`scripts/`内のsidecarビルド処理

import alias は `@client/*`、`@server/*`、`@shared/*` を使う。

## 開発コマンド

- `npm run local` — Bun API を起動
- `npm run dev` — Vite 開発サーバーを起動
- `npm run tauri:dev` — Tauri アプリを開発起動
- `npm run build:desktop` — SPA と Bun sidecar をビルド
- `npm run tauri:build` — macOS `.app` を生成
- `npm run tauri:dmg` — DMG を生成

実行時 DB は既定で `~/.track/track.db`。Tauri はresourceディレクトリと待受ポートを環境変数でsidecarに渡す。利用端末にNode.jsやBunは不要。

本番Tauriアプリは空きloopbackポートを動的に選び、Single Instanceプラグインで二重起動を防ぐ。sidecar起動失敗はDialogプラグインでネイティブ表示する。localhostで配信される設定画面には、バックアップ先フォルダを選ぶ`dialog:allow-open`権限だけを個別に付与する。
