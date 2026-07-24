---
name: track
description: "現在のCodexタスクにおける直近の作業を、起動中の個人用工数管理アプリTrackへローカルAPI経由で記録する。ユーザーが$trackを呼び出したとき、現在の作業を記録するよう依頼したとき、またはセッションからTrackのエントリ案を作るよう求めたときに使用する。作成前に必ず登録案を提示し、明示的な確認を得る。"
---

# Track

1. 作業中のディレクトリから、読み取り専用の準備コマンドを実行する。

   ```bash
   TRACK_CLI=$(/usr/bin/plutil -extract cliPath raw ~/.track/runtime.json)
   "$TRACK_CLI" prepare --source codex
   ```

2. 返されたJSONから時間帯、プロジェクト、重複を確認する。タイトルは現在の会話から簡潔に作る。プロジェクトの根拠が弱ければ「プロジェクトなし」にする。
3. 日付、時間帯、所要時間、プロジェクト、タイトル、重複を提示し、登録前に明示的な確認を得る。`crossesJstMidnight`が`true`なら同一日内の複数エントリに分ける。
4. 確認後に登録する。

   ```bash
   TRACK_CLI=$(/usr/bin/plutil -extract cliPath raw ~/.track/runtime.json)
   "$TRACK_CLI" create \
     --start '<ISO datetime>' --end '<ISO datetime>' \
     --title '<title>' --project-id '<project id>' --confirmed
   ```

   プロジェクトなしなら`--project-id`を省略する。重複をユーザーが許可した場合だけ`--allow-overlap`を加える。
5. 作成されたID、時間帯、プロジェクト、タイトルを報告する。

## エラー

- `cliPath`を取得できない: Trackアプリを起動する。項目自体がなければTrackを更新する。
- `network_error`: Trackアプリを起動してから再実行する。
- `invalid_runtime` / `invalid_api_base`: `~/.track/runtime.json`を削除してTrackを再起動する。
- `exact_duplicate`: 再登録しない。
- `overlap_requires_confirmation`: 重複を提示して続行するか確認する。
- セッションを特定できない場合: 推測せず時間帯を確認する。
