# Track — 工数管理アプリ

## 技術スタック
- **フロントエンド**: Vite + React 19 + TailwindCSS v4 + Zustand + TanStack Query
- **バックエンド**: Cloudflare Workers + Hono
- **DB**: Cloudflare D1 (SQLite) — マイグレーションは `migrations/` に手書きSQL
- **認証**: パスキー (WebAuthn via @simplewebauthn)
- **デプロイ**: `npm run deploy` (vite build → wrangler deploy)

## 構成
- `src/` — React SPA (pages: Calendar, Projects, Reports, Account, Login, Signup)
- `worker/` — Hono APIサーバー (routes: entries, projects, tags, reports, account, invitations)
- `migrations/` — D1マイグレーション (手書きSQL, `npm run db:migrate:local` で適用)
- `wrangler.jsonc` — Cloudflare設定

## 開発コマンド
- `npm run dev` — Vite devサーバー
- `npm run dev:worker` — Wrangler devサーバー
- `npm run db:migrate:local` — ローカルDBマイグレーション
- `npm run deploy` — ビルド＆デプロイ
