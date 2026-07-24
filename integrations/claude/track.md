---
description: Track (個人の工数管理アプリ) に、現セッションの作業を記録する
allowed-tools: Bash, AskUserQuestion
---

# /track — 今のセッションをTrackに記録

`$ARGUMENTS`はプロジェクト、時間調整、タイトルなどの追加ヒントとして解釈する。

## 手順

1. 作業ディレクトリから読み取り専用の準備コマンドを実行する。

   ```bash
   TRACK_CLI=$(/usr/bin/plutil -extract cliPath raw ~/.track/runtime.json)
   "$TRACK_CLI" prepare --source claude
   ```

2. JSONの時間帯、プロジェクト一覧、重複を確認する。現在の会話からタイトルを作り、根拠が弱ければプロジェクトなしにする。
3. 時間帯、所要時間、プロジェクト、タイトル、重複を`AskUserQuestion`で提示し、確認を得る。
4. 確認後に登録する。

   ```bash
   TRACK_CLI=$(/usr/bin/plutil -extract cliPath raw ~/.track/runtime.json)
   "$TRACK_CLI" create \
     --start '<ISO datetime>' --end '<ISO datetime>' \
     --title '<title>' --project-id '<project id>' --confirmed
   ```

   プロジェクトなしなら`--project-id`を省略する。重複を許可された場合だけ`--allow-overlap`を加える。
5. 作成されたID、時間帯、プロジェクト、タイトルを報告する。

Track APIへ接続できない場合は、Trackアプリを起動してから再実行するよう案内する。
接続先は`~/.track/runtime.json`から自動検出される。
`cliPath`がない場合は、同梱CLI対応版のTrackへ更新するよう案内する。
