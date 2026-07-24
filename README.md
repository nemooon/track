# Track — 工数管理アプリ

React の画面と Bun のローカル API を Tauri にまとめた、macOS 向けの工数管理アプリです。SQLite データベースは利用者のホームディレクトリに保存します。

## ディレクトリ構成

```text
track/
├── src/
│   ├── client/       # React SPA（画面・コンポーネント・ブラウザ側処理）
│   ├── server/       # Bun + Hono API（ルート・DB・外部予定表）
│   └── shared/       # client/server 共通の型・日付処理・入力検証
├── database/
│   ├── migrations/   # 起動時に順番に適用する手書き SQL
│   └── schema.prisma # Prisma スキーマ
└── src-tauri/        # Tauri の Rustコード・設定・sidecarビルド処理
```

## セットアップ

必要なものは Node.js、Bun、Rust です。

```bash
npm install
```

Prisma CLI を使う場合は、プロジェクトルートに `.env` を作成します。

```env
DATABASE_URL=file:./database/dev.db
```

その後、クライアントを生成します。

```bash
npx prisma generate
```

## 開発

ブラウザで開発するときは、ターミナルを2つ使います。

```bash
# Bun API（http://127.0.0.1:8787）
npm run local

# Vite（http://localhost:5173）
npm run dev
```

Vite は `/api` を Bun API にプロキシします。ローカル実行時のデータは既定で `~/.track/track.db` に保存されます。

デスクトップアプリとして開発する場合は次を使います。

```bash
npm run tauri:dev
```

APIとViteも同時に起動し、フロントエンドの変更はTauriウィンドウへ
ホットリロードされます。別途`npm run local`や`npm run dev`を起動する必要はありません。

## ビルド

```bash
# Bun sidecar を同梱した macOS .app
npm run tauri:build

# 配布用 DMG
npm run tauri:dmg
```

`.app` は `src-tauri/target/release/bundle/macos/Track.app` に生成されます。Bun の実行環境、React のビルド成果物、SQLite マイグレーションを同梱するため、利用端末への Node.js や Bun のインストールは不要です。

現在のsidecarは`aarch64-apple-darwin`向けで最低動作環境がmacOS 13.0のため、配布対象はmacOS Ventura以降のApple Silicon Macです。Tauri側の`LSMinimumSystemVersion`もsidecarに合わせて13.0に設定しています。

本番アプリは空いているlocalhostポートを起動時に選びます。二度起動した場合は新しいプロセスを増やさず、既存ウィンドウを前面へ戻します。sidecarやDBの起動に失敗した場合は、macOSのエラーダイアログに原因を表示します。

バックアップ先は設定画面の「選択」ボタンからmacOS標準のフォルダ選択ダイアログで指定できます。ブラウザ開発時は従来どおりパスを直接入力します。

カレンダーとレポートはヘッダー左側、または `⌘1`（カレンダー）・`⌘2`（レポート）で切り替えます。ヘッダー中央には画面固有の操作、右側には日付選択とアプリメニューを表示します。表示期間は `⌘[`・`⌘]` で前後へ移動し、`⌘T` で今日へ戻れます。表示中の期間に今日が含まれる場合は「今日」ボタンを強調します。カレンダーの時間軸は `⌘+`・`⌘-` で拡大縮小できます。macOSのメニューバーは「ファイル」「編集」「表示」「ウインドウ」「ヘルプ」を含めて日本語で表示します。設定は「Track」メニューにある「設定…」または `⌘,` でオーバーレイ表示し、勤務・プロジェクト・タグ・バックアップをまとめて管理します。

週表示のレポートでは「週報を生成」から、表示中の絞り込みに一致する工数を
Apple Intelligenceの端末内モデルで週報にできます。出力形式は設定の「週報」で
編集でき、`{{期間}}`と`{{合計時間}}`をプレースホルダーとして使えます。
生成はmacOS 26以降のApple Intelligence対応Macでのみ利用でき、APIキーや通信は
不要です。生成結果はダイアログ内で編集してからコピーできます。

ヘッダーの操作部品以外の空白は、ウインドウを移動するドラッグ領域として動作します。
ヘッダーは緑がかったチャコールグレーを基調にし、操作グループは半透明、選択中の項目は明るい面で表示します。

macOSアプリの開発言語は`src-tauri/Info.plist`で日本語に設定し、OSが「ウインドウ」メニューへ自動追加するタイル表示やディスプレイ移動などの項目も日本語で表示します。

## Homebrewでの配布

配布開始後は、`hako`・`neruna`と同じtapからインストールできます。

```bash
brew install --cask nemooon/tap/track
```

現時点のアプリはApple Developer IDではなくad-hoc署名です。macOSに初回起動を止められた場合は、次のコマンドで隔離属性を削除します。

```bash
xattr -dr com.apple.quarantine /Applications/Track.app
```

リリースの流れは次のとおりです。

1. `npm run version:set -- 0.3.0`で`package.json`、Tauri、Cargoのバージョンをまとめて更新する
2. `npm run version:check`でバージョンの一致を確認する
3. 変更をコミットし、GitHubへpushする
4. `npm run release`でApple Silicon向け`.app`をビルドし、`Track-<version>.zip`をGitHub Releaseへ公開する
5. `.github/workflows/bump-cask.yml`がsha256を計算し、`nemooon/homebrew-tap`の`Casks/track.rb`を更新する

リリースノートを指定する場合は`npm run release -- path/to/notes.md`を使います。省略時はGitHubが前回のタグとの差分からリリースノートを生成します。zipだけをローカルで確認するときは`npm run release:package`を使います。

初回リリース前に、次の準備が必要です。

- `nemooon/track`をpublicリポジトリにする（通常のHomebrew CaskはprivateなGitHub Releaseを取得できません）
- TrackリポジトリのActions secret `TAP_GITHUB_TOKEN`に、`nemooon/homebrew-tap`へ書き込めるfine-grained PATを登録する
- GitHub Actionsを有効にする

Homebrew Caskの原型は`packaging/homebrew/track.rb.template`に置いてあります。release workflowは実際のバージョンとsha256を埋め、tap側のCaskを新規作成または更新します。

## コマンド

| コマンド | 内容 |
|---|---|
| `npm run dev` | Vite 開発サーバーを起動 |
| `npm run dev:desktop` | Tauri開発用にAPIとViteを同時起動 |
| `npm run local` | Bun API を起動 |
| `npm run cli -- <command>` | 開発中のAI連携CLIを実行 |
| `npm run build` | React SPA をビルド |
| `npm run build:ai-helper` | Apple Intelligence用のmacOSヘルパーをビルド |
| `npm run build:cli` | AIツール連携CLIをビルド |
| `npm run build:sidecar` | Bun API をTauri用の単一実行ファイルへコンパイル |
| `npm run build:desktop` | SPA と sidecar をビルド |
| `npm run tauri:dev` | Tauri アプリを開発起動 |
| `npm run tauri:build` | macOS `.app` をビルド |
| `npm run tauri:dmg` | DMG をビルド |
| `npm run version:check` | 配布に関係する全ファイルのバージョン一致を確認 |
| `npm run version:set -- <version>` | 配布に関係する全ファイルのバージョンを更新 |
| `npm run release:package` | Homebrew配布用のzipをローカル生成 |
| `npm run release` | zipをビルドし、GitHub Releaseとして公開 |

## Codex / Claude Codeから工数を記録

TrackにはAIツール連携専用のCLIを`Track.app/Contents/MacOS/track-cli`として
同梱します。ユーザー向けコマンドとしてPATHへインストールはしません。
Trackの起動中はローカルAPIと同梱CLIの実体パスを`~/.track/runtime.json`へ
自動的に書き出します。ファイルのパーミッションは所有者のみ読み書き可能な
`0600`です。

Codex用スキルとClaude Code用コマンドもTrack.appへ同梱します。macOSの「Track」
メニューから「AI連携をインストール…」を選ぶと、次の場所へインストールします。

- Codex: `~/.codex/skills/track`
- Claude Code: `~/.claude/commands/track.md`

再実行するとTrackの連携ファイルだけを最新版へ更新します。同名の無関係なスキルや
コマンドは上書きしません。インストール後、新しいCodexまたはClaude Codeセッション
から利用できます。

どちらも`runtime.json`から同梱CLIを特定して直接実行し、現在のセッションから
作業時間帯を推定します。プロジェクト・タイトル・既存エントリとの重複を提示し、
利用者が明示的に確認した後にだけ登録します。

- Codex: 「`$track`で今の作業を記録して」
- Claude Code: 「`/track`」

AIツールが内部で同梱CLIを取得する例です。通常、利用者がこのコマンドを直接
実行する必要はありません。

```bash
TRACK_CLI=$(/usr/bin/plutil -extract cliPath raw ~/.track/runtime.json)
"$TRACK_CLI" prepare --source codex
```

開発時は`npm run cli -- <command>`で直接実行できます。

## Apple Intelligenceサンプル

macOS 26以降のFoundation Modelsを使い、端末内だけで週報の下書きを生成する
サンプルを`/samples/apple-intelligence`に置いています。APIキーや通信は不要です。
コマンドは一時的なad-hoc署名のmacOSアプリを`.build`へ作り、画面を表示せずに
LaunchServices経由で実行します。

モデルが利用可能かだけを確認します。

```bash
npm run sample:apple-intelligence -- --check
```

同梱した工数例から週報を生成します。

```bash
npm run sample:apple-intelligence
```

任意の工数テキストを渡すこともできます。

```bash
npm run sample:apple-intelligence -- \
  "月曜: A社定例 1時間。火曜: レポート画面の改善 4時間。"
```

実行にはmacOS 26 SDKを含むXcodeまたはCommand Line Toolsと、Apple Intelligenceが
有効な対応Macが必要です。生成内容はモデルによって変わるため、事実確認してから
利用してください。
